//! Primary CLI startup stays owned by the job while early control remains available.
use crate::helpers::{KillVms, config, helper, repo_with_bundle, workspace};
use agentvm::adapters::{
    control,
    pty::PtyConnection,
    vm::{VmEvent, VmProcess},
};
use agentvm::domain::{
    agents::{AgentKind, AgentSelection},
    custom_cli::CustomCli,
    spec::TaskSpec,
};
use serde_json::json;
use std::time::{Duration, Instant};

#[tokio::test]
#[ignore = "requires the golden image; no inference quota"]
async fn an_early_primary_terminal_does_not_interrupt_cli_installation() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    let base = repo_with_bundle(&ws);
    ws.write_spec(&TaskSpec {
        id: "early-cli".into(),
        prompt: "first prompt".into(),
        branch: "agent/early-cli".into(),
        base_sha: base,
        timeout_s: 60,
        interactive: true,
        agent: AgentSelection {
            kind: AgentKind::Custom,
            custom: Some(CustomCli {
                name: "Root shell".into(),
                install: Some("touch /mnt/job/installing; sleep 4".into()),
                interactive: "/bin/bash --noprofile --norc".into(),
                headless: None,
                resume: None,
            }),
            ..Default::default()
        },
        agent_endpoint: None,
        agent_native_defaults: None,
        agent_protocol: None,
        model: None,
        claude_version: None,
        restore: false,
    })
    .unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.events()).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ws.share().join("installing").exists() {
        assert!(Instant::now() < deadline, "installer did not start");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let mut early = PtyConnection::open(&ws.pty_socket(), "claude", 100, 30).await.unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(2), early.recv()).await;
    let sessions = control::request(&ws.pty_socket(), "terminal.list", json!({})).await.unwrap();
    let started_early = sessions["sessions"].as_array().unwrap().contains(&json!("claude"));
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline && !ws.job_log().iter().any(|line| line.ends_with("terminal ready")) {
        if ws.share().join("result.json").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let ready = ws.job_log().iter().any(|line| line.ends_with("terminal ready"));
    let probe = control::request(
        &ws.pty_socket(),
        "exec.start",
        json!({
            "argv": ["/bin/bash", "-c", "! ss -ltnH '( sport = :5001 )' | grep -q ."],
        }),
    )
    .await
    .unwrap();
    let relay_absent = loop {
        let result =
            control::request(&ws.pty_socket(), "exec.read", json!({"exec_id":probe["exec_id"]})).await.unwrap();
        if result["state"] != "running" {
            break result["exit_code"] == 0;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    vm.terminate();
    vm.wait().await;
    assert!(!started_early, "an attach created the primary CLI before its installer finished");
    assert!(ready, "early attachment interrupted initialization: {:?}", ws.job_log());
    assert!(relay_absent, "a credential-free CLI should leave development port 5001 available");
}
