//! Read a real account catalog from a guest without sending an inference request.
use crate::helpers::{KillVms, config, helper, repo_with_bundle, shell, workspace};
use agentvm::adapters::{
    agent_proxy::ProviderRelay,
    keychain::Keychain,
    vm::{VmEvent, VmProcess},
};
use agentvm::app::{accounts, supervisor::AppCtx};
use agentvm::domain::{
    accounts::AccountKind,
    agents::{AgentKind, AgentSelection},
    custom_cli::CustomCli,
    spec::TaskSpec,
};
use std::{process::Command, sync::Arc, time::Duration};

struct PrivateKeychain(std::path::PathBuf);
impl Drop for PrivateKeychain {
    fn drop(&mut self) {
        let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
    }
}

#[tokio::test]
#[ignore = "requires golden image and a current host Codex ChatGPT login; no inference quota"]
async fn guest_discovers_real_chatgpt_models_with_host_only_read_only_auth() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let private = PrivateKeychain(tmp.path().join("catalog.keychain-db"));
    assert!(
        Command::new("security").args(["create-keychain", "-p", "test"]).arg(&private.0).status().unwrap().success()
    );
    let host_config = agentvm::config::Config {
        home: tmp.path().join("engine"),
        port: 0,
        concurrency: 1,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 60,
        vm_helper: helper(),
        scripts_dir: "scripts".into(),
        min_free_mb: 0,
    };
    let ctx = Arc::new(AppCtx::new(host_config, Keychain::new(Some(private.0.clone()))));
    accounts::import(&ctx, AccountKind::Chatgpt).expect("sign in with Codex on the host before this test");
    let access_ctx = ctx.clone();
    let ws = workspace(&tmp);
    let _relay = ProviderRelay::start_account(
        &ws.agent_socket(),
        AccountKind::Chatgpt,
        accounts::config(AccountKind::Chatgpt),
        Arc::new(move |force| {
            let ctx = access_ctx.clone();
            Box::pin(async move { accounts::access(&ctx, AccountKind::Chatgpt, force).await })
        }),
    )
    .unwrap();
    let base = repo_with_bundle(&ws);
    ws.write_spec(&TaskSpec {
        id: "account-catalog".into(),
        prompt: String::new(),
        branch: "agent/account-catalog".into(),
        base_sha: base,
        timeout_s: 60,
        interactive: true,
        agent: AgentSelection {
            kind: AgentKind::Custom,
            custom: Some(CustomCli {
                name: "Catalog shell".into(),
                install: None,
                interactive: "/bin/bash --noprofile --norc".into(),
                headless: None,
                resume: None,
            }),
            ..Default::default()
        },
        agent_endpoint: Some("http://127.0.0.1:5001/backend-api/codex".into()),
        agent_native_defaults: None,
        agent_protocol: None,
        model: None,
        claude_version: None,
        restore: false,
    })
    .unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    cfg.agent_socket = Some(ws.agent_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.events()).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    let command = "python3 -c 'import sys; sys.path.insert(0,\"/mnt/job/runtime\"); from agent_models import discover; m=discover({\"agent\":{\"kind\":\"codex\",\"auth\":\"account\",\"account_kind\":\"chatgpt\"},\"agent_endpoint\":\"http://127.0.0.1:5001/backend-api/codex\"}); print(\"ACCOUNT_CATALOG_OK=\"+m)'";
    let output = shell(&ws.pty_socket(), command).await;
    vm.terminate();
    tokio::time::timeout(Duration::from_secs(15), vm.wait()).await.unwrap();
    assert!(output.contains("ACCOUNT_CATALOG_OK=gpt-"), "account catalog discovery failed: {output}");
    for file in ["task.json", ".token", "job.log", "stream.jsonl"] {
        let content = std::fs::read_to_string(ws.share().join(file)).unwrap_or_default();
        let host = accounts::access(&ctx, AccountKind::Chatgpt, false).await.unwrap();
        assert!(!content.contains(host.credential.expose()), "host credential entered {file}");
    }
}
