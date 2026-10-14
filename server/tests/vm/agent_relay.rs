use crate::helpers::{KillVms, config, helper, repo_with_bundle, shell, workspace};
use agentvm::adapters::keychain::Keychain;
use agentvm::adapters::vm::{VmEvent, VmProcess};
use agentvm::app::supervisor::AppCtx;
use agentvm::app::{agent_gateway, record::TaskRecord};
use agentvm::domain::agents::{AgentKind, AgentSelection};
use agentvm::domain::custom_cli::CustomCli;
use agentvm::domain::ids::{CommitSha, RepoPath, TaskId};
use agentvm::domain::providers::{ApiProtocol, Provider, ProviderConfig};
use agentvm::domain::spec::TaskSpec;
use agentvm::secret::Secret;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires the golden image and signed VM helper; no model inference"]
async fn guest_reaches_host_relay_and_reconnects_after_it_restarts() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let host_config = agentvm::config::Config {
        home: tmp.path().join("engine"),
        port,
        concurrency: 1,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 60,
        vm_helper: helper(),
        scripts_dir: "scripts".into(),
        min_free_mb: 0,
    };
    let ctx = Arc::new(AppCtx::new(host_config, Keychain::new(Some(tmp.path().join("none.keychain-db")))));
    let engine_ctx = ctx.clone();
    let engine = tokio::spawn(async {
        axum::serve(listener, agentvm::http::router(engine_ctx)).await.unwrap();
    });
    let ws = workspace(&tmp);
    let base = repo_with_bundle(&ws);
    let provider = ProviderConfig {
        endpoint: format!("http://127.0.0.1:{port}/api"),
        protocol: ApiProtocol::Openai,
        requires_key: false,
    };
    let id = TaskId::parse(ws.dir().file_name().unwrap().to_str().unwrap()).unwrap();
    let record_repo = tmp.path().join("relay-record-repo");
    std::fs::create_dir(&record_repo).unwrap();
    assert!(std::process::Command::new("git").args(["init", "-q"]).arg(&record_repo).status().unwrap().success());
    let mut record =
        TaskRecord::new(id, RepoPath::new(record_repo).unwrap(), None, CommitSha::parse(&base).unwrap(), true);
    record.agent = AgentSelection { kind: AgentKind::Custom, provider: Some(Provider::Custom), ..Default::default() };
    record.agent_provider_config = Some(provider.clone());
    let relay = agent_gateway::start(&ctx, &record, &Secret::new(String::new())).unwrap();
    ws.write_spec(&TaskSpec {
        id: "t".into(),
        prompt: String::new(),
        branch: "agent/t".into(),
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
        agent_endpoint: Some("http://127.0.0.1:5001/api".into()),
        agent_native_defaults: None,
        agent_protocol: Some(ApiProtocol::Openai),
        model: None,
        claude_version: None,
        restore: false,
    })
    .unwrap();
    ws.write_token(&Secret::new("sk-ant-oat01-agentvm-dummy".into())).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    cfg.agent_socket = Some(ws.agent_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.events()).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    let before = shell(&ws.pty_socket(), "curl -fsS --max-time 5 http://127.0.0.1:5001/api/settings").await;
    drop(relay);
    let mut changed = ctx.settings.get();
    changed.providers.insert(
        Provider::Custom,
        ProviderConfig {
            endpoint: format!("http://127.0.0.1:{port}/api/v2"),
            protocol: ApiProtocol::Google,
            requires_key: false,
        },
    );
    ctx.settings.update(changed).unwrap();
    // Recovery reloads the task record while settings may have changed during downtime.
    let recovered: TaskRecord = serde_json::from_slice(&serde_json::to_vec(&record).unwrap()).unwrap();
    let _relay = agent_gateway::start(&ctx, &recovered, &Secret::new(String::new())).unwrap();
    let after = shell(&ws.pty_socket(), "curl -fsS --max-time 5 http://127.0.0.1:5001/api/settings").await;
    vm.terminate();
    tokio::time::timeout(Duration::from_secs(15), vm.wait()).await.unwrap();
    engine.abort();
    assert!(before.contains("\"timeout_s\":60"), "relay before restart: {before}");
    assert!(after.contains("\"timeout_s\":60"), "relay after restart: {after}");
}
