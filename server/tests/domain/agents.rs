//! Selection persistence and launch validation.
use agentvm::domain::settings::{HostLimits, Settings};
use serde_json::json;

fn settings(agent: serde_json::Value) -> serde_json::Value {
    json!({"max_vms":1,"cpus":1,"memory_mb":2048,"timeout_s":60,"model":"opus","default_repo":null,"agent":agent})
}

#[test]
fn selected_agent_round_trips_and_old_settings_default_to_claude() {
    let explicit: Settings = serde_json::from_value(settings(json!({"kind":"codex"}))).unwrap();
    assert_eq!(serde_json::to_value(explicit).unwrap()["agent"]["kind"], "codex");
    let mut old = settings(json!({"kind":"codex"}));
    old.as_object_mut().unwrap().remove("agent");
    let old: Settings = serde_json::from_value(old).unwrap();
    assert_eq!(serde_json::to_value(old).unwrap()["agent"]["kind"], "claude");
}

#[test]
fn selection_rejects_incompatible_configured_protocol() {
    let mut value = settings(json!({"kind":"codex","provider":"custom"}));
    value["providers"] =
        json!({"custom":{"endpoint":"https://example.org/v1","protocol":"anthropic","requires_key":false}});
    let s: Settings = serde_json::from_value(value).unwrap();
    assert!(s.validate(&HostLimits { cpus: 4, ram_mb: 32768 }).is_err());
}

#[test]
fn codex_rejects_coding_plan_endpoints_regardless_of_provider_alias() {
    use agentvm::domain::agents::{AgentKind, AgentSelection};
    use agentvm::domain::providers::{ApiProtocol, Provider, ProviderConfig};
    for host in ["coding.dashscope.aliyuncs.com", "coding-intl.dashscope.aliyuncs.com"] {
        for path in ["/v1", "/v1/", "/apps/anthropic", "/apps/anthropic/v1"] {
            let config = ProviderConfig {
                endpoint: format!("https://{host}{path}"),
                protocol: ApiProtocol::Openai,
                requires_key: true,
            };
            for provider in [Provider::Alibabacodingplan, Provider::Custom, Provider::Openai] {
                let agent = AgentSelection { kind: AgentKind::Codex, provider: Some(provider), ..Default::default() };
                assert!(agent.validate(Some(&config), true).is_err(), "{} accepted {provider:?}", config.endpoint);
            }
        }
    }
}

#[test]
fn codex_compatibility_uses_the_endpoint_rather_than_the_provider_id() {
    use agentvm::domain::agents::{AgentKind, AgentSelection};
    use agentvm::domain::providers::{ApiProtocol, Provider, ProviderConfig};
    for endpoint in [
        "http://127.0.0.1:11434/v1",
        "https://coding-intl.dashscope.aliyuncs.com.example.org/v1",
        "https://example.org/coding-intl.dashscope.aliyuncs.com/v1",
        "https://coding-intl.dashscope.aliyuncs.com/compatible-mode/v1",
    ] {
        let config = ProviderConfig { endpoint: endpoint.into(), protocol: ApiProtocol::Openai, requires_key: false };
        let agent = AgentSelection {
            kind: AgentKind::Codex,
            provider: Some(Provider::Alibabacodingplan),
            ..Default::default()
        };
        assert!(agent.validate(Some(&config), true).is_ok(), "{endpoint}");
    }
}

#[test]
fn custom_automatic_tasks_require_a_headless_template() {
    let agent: agentvm::domain::agents::AgentSelection = serde_json::from_value(
        json!({"kind":"custom","custom":{"name":"Example","interactive":"example --prompt {prompt}"}}),
    )
    .unwrap();
    assert!(agent.validate(None, true).is_ok());
    assert!(agent.validate(None, false).unwrap_err().contains("headless"));
}

#[test]
fn custom_template_refuses_an_option_as_the_executable() {
    let agent: agentvm::domain::agents::AgentSelection =
        serde_json::from_value(json!({"kind":"custom","custom":{"name":"Example","interactive":"--evil {prompt}"}}))
            .unwrap();
    assert!(agent.validate(None, true).is_err());
}

#[test]
fn reusable_custom_commands_are_validated_before_saving_settings() {
    let mut value = settings(json!({"kind":"claude"}));
    value["custom_cli"] = json!({"name":"Example","interactive":"example; rm /tmp/x"});
    let s: Settings = serde_json::from_value(value).unwrap();
    assert!(s.validate(&HostLimits { cpus: 4, ram_mb: 32768 }).is_err());
}

#[test]
fn all_agents_accept_automatic_model_selection() {
    use agentvm::domain::agents::{AgentKind, AgentSelection};
    use agentvm::domain::settings::Model;
    for kind in [AgentKind::Claude, AgentKind::Codex, AgentKind::Gemini] {
        assert!(AgentSelection { kind, ..Default::default() }.validate_model(&Model::default()).is_ok());
    }
    for kind in
        [AgentKind::Opencode, AgentKind::Qwen, AgentKind::Copilot, AgentKind::Kilo, AgentKind::Aider, AgentKind::Goose]
    {
        let selection = AgentSelection { kind, ..Default::default() };
        assert!(selection.validate_model(&Model::default()).is_ok());
        assert!(selection.validate_model(&Model::parse("openai/GPT-5:free").unwrap()).is_ok());
    }
}

#[test]
fn custom_templates_accept_automatic_model_selection() {
    use agentvm::domain::agents::AgentSelection;
    use agentvm::domain::settings::Model;
    let selection: AgentSelection = serde_json::from_value(
        json!({"kind":"custom","custom":{"name":"Example","interactive":"example --model {model}"}}),
    )
    .unwrap();
    assert!(selection.validate_model(&Model::default()).is_ok());
    assert!(selection.validate_model(&Model::parse("Explicit/Model:free").unwrap()).is_ok());
}
