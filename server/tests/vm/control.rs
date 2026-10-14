//! Structured vsock control on a real isolated VM, without opening an inference session.
use crate::helpers::{KillVms, config, helper, repo_with_bundle, workspace};
use agentvm::adapters::control;
use agentvm::adapters::vm::{VmEvent, VmProcess};
use agentvm::domain::spec::TaskSpec;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

async fn finished(socket: &std::path::Path, id: &str) -> Value {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let result = control::request(socket, "exec.read", json!({"exec_id":id})).await.unwrap();
        if result["state"] != "running" {
            return result;
        }
        assert!(Instant::now() < until, "execution never finished");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
#[ignore = "requires rebuilt guest runtime, helper and golden image; no inference quota"]
async fn structured_vm_control_exec_files_and_persistent_tmux() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    let base = repo_with_bundle(&ws);
    ws.write_spec(&TaskSpec {
        id: "control-test".into(),
        prompt: String::new(),
        branch: "agent/control-test".into(),
        base_sha: base,
        timeout_s: 60,
        interactive: true,
        agent: Default::default(),
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
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.dir().join("vm.events")).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    let ready = Instant::now() + Duration::from_secs(60);
    loop {
        if control::request(&ws.pty_socket(), "capabilities", json!({})).await.is_ok() {
            break;
        }
        assert!(Instant::now() < ready, "structured runtime never answered");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let started = control::request(
        &ws.pty_socket(),
        "exec.start",
        json!({"argv":["/bin/echo","literal ; $(touch /root/BAD)"],"cwd":"/root/work"}),
    )
    .await
    .unwrap();
    let result = finished(&ws.pty_socket(), started["exec_id"].as_str().unwrap()).await;
    assert_eq!(result["stdout"], "bGl0ZXJhbCA7ICQodG91Y2ggL3Jvb3QvQkFEKQo=");
    assert_eq!(result["exit_code"], 0);
    let sleep =
        control::request(&ws.pty_socket(), "exec.start", json!({"argv":["sleep","30"],"timeout_s":1})).await.unwrap();
    assert_eq!(finished(&ws.pty_socket(), sleep["exec_id"].as_str().unwrap()).await["state"], "timed_out");
    let sleep = control::request(&ws.pty_socket(), "exec.start", json!({"argv":["sleep","30"]})).await.unwrap();
    control::request(&ws.pty_socket(), "exec.cancel", json!({"exec_id":sleep["exec_id"]})).await.unwrap();
    assert_eq!(finished(&ws.pty_socket(), sleep["exec_id"].as_str().unwrap()).await["state"], "cancelled");
    control::request(
        &ws.pty_socket(),
        "files.write",
        json!({"path":"/root/work/binary","data":"AP9B","truncate":true}),
    )
    .await
    .unwrap();
    let file =
        control::request(&ws.pty_socket(), "files.read", json!({"path":"/root/work/binary","offset":1,"size":2}))
            .await
            .unwrap();
    assert_eq!(file["data"], "/0E=");
    control::request(&ws.pty_socket(), "terminal.create", json!({"session":"shell-2"})).await.unwrap();
    control::request(
        &ws.pty_socket(),
        "terminal.input",
        json!({"session":"shell-2","text":"printf literal-tmux","keys":["Enter"]}),
    )
    .await
    .unwrap();
    control::request(&ws.pty_socket(), "terminal.resize", json!({"session":"shell-2","cols":90,"rows":25}))
        .await
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let capture =
            control::request(&ws.pty_socket(), "terminal.capture", json!({"session":"shell-2"})).await.unwrap();
        if capture["text"].as_str().unwrap().contains("literal-tmux") {
            break;
        }
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let sessions = control::request(&ws.pty_socket(), "terminal.list", json!({})).await.unwrap();
    assert!(sessions["sessions"].as_array().unwrap().iter().any(|s| s == "shell-2"));
    vm.terminate();
    vm.wait().await;
}
