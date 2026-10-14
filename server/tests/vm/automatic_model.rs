//! Real public provider discovery through the private relay, without inference calls.
use crate::helpers::{KillVms, config, helper, repo_with_bundle, workspace};
use agentvm::adapters::{
    agent_proxy::ProviderRelay,
    vm::{VmEvent, VmProcess},
};
use agentvm::domain::{
    agents::{AgentKind, AgentSelection},
    custom_cli::CustomCli,
    providers::{ApiProtocol, ProviderConfig},
    spec::TaskSpec,
};
use agentvm::secret::Secret;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires the golden image and public provider network; no inference quota"]
async fn automatic_model_selection_uses_real_provider_catalog_without_inference() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    let base = repo_with_bundle(&ws);
    let _relay = ProviderRelay::start(
        &ws.agent_socket(),
        ProviderConfig {
            endpoint: "https://openrouter.ai/api/v1".into(),
            protocol: ApiProtocol::Openai,
            requires_key: false,
        },
        Secret::new(String::new()),
        false,
    )
    .unwrap();
    ws.write_spec(&TaskSpec {
        id: "auto-model".into(),
        prompt: "".into(),
        branch: "agent/auto-model".into(),
        base_sha: base,
        timeout_s: 60,
        interactive: false,
        agent: AgentSelection {
            kind: AgentKind::Custom,
            custom: Some(CustomCli {
                name: "Model catalog check".into(),
                install: None,
                interactive: "/bin/bash --noprofile --norc".into(),
                headless: Some("/bin/echo '{model}'".into()),
                resume: None,
            }),
            ..Default::default()
        },
        agent_endpoint: Some("http://127.0.0.1:5001/api/v1".into()),
        agent_native_defaults: None,
        agent_protocol: Some(ApiProtocol::Openai),
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
    let outcome = tokio::time::timeout(Duration::from_secs(60), vm.wait()).await;
    if outcome.is_err() {
        vm.terminate();
        vm.wait().await;
    }
    assert!(outcome.is_ok(), "automatic configuration did not complete");
    let result = ws.read_result().unwrap();
    assert_eq!(result.claude_exit, 0, "{:?}: {:?}", result, ws.job_log());
    let stream = std::fs::read_to_string(ws.share().join("stream.jsonl")).unwrap();
    let events = stream.lines().flat_map(agentvm::domain::agent_event::parse_line).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| match event {
            agentvm::domain::agent_event::AgentEvent::Text { text } => text.contains('/') && !text.contains("dummy"),
            _ => false,
        }),
        "no automatic provider model in {events:?}"
    );
}
