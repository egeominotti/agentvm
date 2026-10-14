//! Install and inspect actual ARM64 CLI binaries without model inference.
use crate::helpers::{KillVms, config, helper, repo_with_bundle, workspace};
use agentvm::adapters::{
    control,
    vm::{VmEvent, VmProcess},
};
use agentvm::domain::{
    agents::{AgentKind, AgentSelection},
    custom_cli::CustomCli,
    spec::TaskSpec,
};
use agentvm::secret::Secret;
use serde_json::json;
use std::time::{Duration, Instant};

#[tokio::test]
#[ignore = "downloads actual CLI packages in a disposable VM; no inference quota"]
async fn all_supported_cli_binaries_install_and_expose_real_launch_flags() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    let base = repo_with_bundle(&ws);
    ws.write_spec(&TaskSpec {
        id: "cli-install".into(),
        prompt: String::new(),
        branch: "agent/cli-install".into(),
        base_sha: base,
        timeout_s: 60,
        interactive: true,
        agent: AgentSelection {
            kind: AgentKind::Custom,
            custom: Some(CustomCli {
                name: "Root shell".into(),
                install: None,
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
    ws.write_token(&Secret::new("sk-ant-oat01-agentvm-dummy".into())).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    cfg.memory_mb = 4096;
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.events()).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    let ready = Instant::now() + Duration::from_secs(30);
    while control::request(&ws.pty_socket(), "capabilities", json!({})).await.is_err() {
        assert!(Instant::now() < ready, "guest control did not start");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let script = r#"set -e
export PATH=/root/.local/bin:$PATH
for cli in claude codex gemini opencode qwen copilot kilo aider goose; do
  jq --arg kind "$cli" '.agent={kind:$kind} | .restore=false' /mnt/job/task.json > /run/agentvm/install-test.json
  /usr/local/bin/agentvm-install /run/agentvm/install-test.json
  "$cli" --version
  "$cli" --help > "/mnt/job/${cli}-help.txt" 2>&1
done
# A restored Goose disk can switch to ChatGPT after Codex was removed.
rm -f /root/.local/bin/codex
jq '.agent={kind:"goose",auth:"account",account_kind:"chatgpt"}' /mnt/job/task.json > /run/agentvm/install-test.json
/usr/local/bin/agentvm-install /run/agentvm/install-test.json
codex --version
test "$(cat /root/.local/share/agentvm/codex-installed)" = 0.162.1
for check in codex:dangerously-bypass-approvals-and-sandbox gemini:approval-mode qwen:yolo copilot:allow-all kilo:auto aider:message-file; do
  echo "checking $check"
  grep -q "${check#*:}" "/mnt/job/${check%%:*}-help.txt"
done
echo CLI_INSTALLS_AND_FLAGS_VERIFIED
"#;
    let started = control::request(
        &ws.pty_socket(),
        "exec.start",
        json!({
            "argv":["/bin/bash","-lc",script],"timeout_s":1800,"output_limit":1048576,
        }),
    )
    .await
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(1900);
    let result = loop {
        let output =
            control::request(&ws.pty_socket(), "exec.read", json!({"exec_id":started["exec_id"]})).await.unwrap();
        if output["state"] != "running" {
            break output;
        }
        assert!(Instant::now() < deadline, "CLI installation did not finish");
        tokio::time::sleep(Duration::from_secs(1)).await;
    };
    vm.terminate();
    vm.wait().await;
    std::fs::write(ws.dir().join("cli-install-result.json"), serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    assert_eq!(result["exit_code"], 0, "CLI install output (base64): {result}");
}
