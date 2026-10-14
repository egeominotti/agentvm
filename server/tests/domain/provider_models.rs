use agentvm::domain::agents::AgentKind;
use agentvm::domain::provider_models::{filter_catalog, preset_catalog};
use agentvm::domain::providers::{ApiProtocol, ProviderConfig};
use serde_json::json;

fn config(endpoint: &str, protocol: ApiProtocol) -> ProviderConfig {
    ProviderConfig { endpoint: endpoint.into(), protocol, requires_key: true }
}

#[test]
fn coding_plan_has_automatic_models_without_an_upstream_model_list_endpoint() {
    for endpoint in ["https://coding.dashscope.aliyuncs.com/v1", "https://coding-intl.dashscope.aliyuncs.com/v1"] {
        let catalog = preset_catalog(&config(endpoint, ApiProtocol::Openai)).unwrap();
        assert!(catalog["data"].as_array().unwrap().iter().any(|model| model["id"] == "qwen3.7-plus"));
    }
    assert!(
        preset_catalog(&config("https://coding-intl.dashscope.aliyuncs.com.evil.test/v1", ApiProtocol::Openai))
            .is_none()
    );
    assert!(preset_catalog(&config("http://127.0.0.1:11434/v1", ApiProtocol::Openai)).is_none());
}

#[test]
fn coding_plan_anthropic_discovery_uses_verified_paths_and_minimal_model_metadata() {
    for host in ["coding.dashscope.aliyuncs.com", "coding-intl.dashscope.aliyuncs.com"] {
        for path in ["/apps/anthropic", "/apps/anthropic/v1"] {
            let endpoint = format!("https://{host}{path}");
            assert_eq!(
                preset_catalog(&config(&endpoint, ApiProtocol::Anthropic)).unwrap(),
                json!({"data":[{"id":"qwen3.7-plus","display_name":"Qwen3.7 Plus","type":"model"}],"has_more":false})
            );
            assert!(preset_catalog(&config(&endpoint, ApiProtocol::Openai)).is_none());
        }
        assert!(preset_catalog(&config(&format!("https://{host}/v1"), ApiProtocol::Anthropic)).is_none());
    }
    assert!(
        preset_catalog(&config(
            "https://coding-intl.dashscope.aliyuncs.com.evil.test/apps/anthropic",
            ApiProtocol::Anthropic
        ))
        .is_none()
    );
    assert!(
        preset_catalog(&config("https://coding-intl.dashscope.aliyuncs.com/apps/anthropic/v2", ApiProtocol::Anthropic))
            .is_none()
    );
}

#[test]
fn opencode_subscription_models_match_the_clis_actual_request_format() {
    let catalog = json!({"data":[{"id":"kimi-k3"},{"id":"glm-5.3"},{"id":"gpt-6-luna"},{"id":"claude-haiku-5-5"}]});
    let chat = config("https://opencode.ai/inference/go/v1", ApiProtocol::Openai);
    let chats = filter_catalog(&chat, AgentKind::Opencode, catalog.clone()).unwrap();
    assert_eq!(chats["data"], json!([{"id":"kimi-k3"},{"id":"glm-5.3"}]));
    let responses = filter_catalog(&chat, AgentKind::Codex, catalog.clone()).unwrap();
    assert_eq!(responses["data"], json!([{"id":"gpt-6-luna"}]));
    let messages = config("https://opencode.ai/zen/go/v1", ApiProtocol::Anthropic);
    assert_eq!(
        filter_catalog(&messages, AgentKind::Claude, catalog.clone()).unwrap()["data"],
        json!([{"id":"claude-haiku-5-5"}])
    );
    assert!(
        filter_catalog(&config("https://api.openai.com/v1", ApiProtocol::Openai), AgentKind::Codex, catalog).is_none()
    );
}

#[test]
fn opencode_go_and_zen_route_qwen_and_minimax_differently() {
    let catalog = json!({"data":[
        {"id":"qwen3.8-max"},{"id":"qwen3.8-flash"},{"id":"minimax-m3"},
        {"id":"minimax-m2.7"},{"id":"minimax-m2.5"},{"id":"glm-5.3"}
    ]});
    for prefix in ["inference", "zen"] {
        let go = format!("https://opencode.ai/{prefix}/go/v1");
        let zen = format!("https://opencode.ai/{prefix}/v1");
        assert_eq!(
            filter_catalog(&config(&go, ApiProtocol::Openai), AgentKind::Opencode, catalog.clone()).unwrap()["data"],
            json!([{"id":"glm-5.3"}])
        );
        assert_eq!(
            filter_catalog(&config(&go, ApiProtocol::Anthropic), AgentKind::Claude, catalog.clone()).unwrap()["data"],
            json!([{"id":"qwen3.8-max"},{"id":"qwen3.8-flash"},{"id":"minimax-m3"},{"id":"minimax-m2.7"}])
        );
        assert_eq!(
            filter_catalog(&config(&zen, ApiProtocol::Openai), AgentKind::Opencode, catalog.clone()).unwrap()["data"],
            json!([{"id":"qwen3.8-max"},{"id":"minimax-m3"},{"id":"minimax-m2.7"},{"id":"minimax-m2.5"},{"id":"glm-5.3"}])
        );
        assert_eq!(
            filter_catalog(&config(&zen, ApiProtocol::Anthropic), AgentKind::Claude, catalog.clone()).unwrap()["data"],
            json!([{"id":"qwen3.8-flash"}])
        );
    }
}

#[test]
fn opencode_responses_routes_include_grok_and_muse() {
    for (path, muse) in [("inference/go", "muse-spark-1.3-contributor"), ("inference", "muse-spark-1.3")] {
        let endpoint = config(&format!("https://opencode.ai/{path}/v1"), ApiProtocol::Openai);
        let catalog = json!({"data":[{"id":"gpt-6-luna"},{"id":"grok-4.7"},{"id":muse},{"id":"kimi-k3"}]});
        assert_eq!(
            filter_catalog(&endpoint, AgentKind::Codex, catalog.clone()).unwrap()["data"],
            json!([{"id":"gpt-6-luna"},{"id":"grok-4.7"},{"id":muse}])
        );
        assert_eq!(filter_catalog(&endpoint, AgentKind::Opencode, catalog).unwrap()["data"], json!([{"id":"kimi-k3"}]));
    }
}

#[test]
fn opencode_omits_unknown_and_system_one_formats_instead_of_assuming_chat() {
    let catalog = json!({"data":[
        {"id":"unknown-model"},{"id":"jev-1.13"},{"id":"jev-1.13-free"},
        {"id":"gpt-unannounced"},{"id":"claude-unannounced"},{"id":"gemini-unannounced"}
    ]});
    for (protocol, kind) in [
        (ApiProtocol::Openai, AgentKind::Opencode),
        (ApiProtocol::Openai, AgentKind::Codex),
        (ApiProtocol::Anthropic, AgentKind::Claude),
        (ApiProtocol::Google, AgentKind::Gemini),
    ] {
        let endpoint = config("https://opencode.ai/inference/v1", protocol);
        assert_eq!(filter_catalog(&endpoint, kind, catalog.clone()).unwrap()["data"], json!([]));
    }
}

#[test]
fn opencode_preserves_documented_chat_and_google_routes() {
    let catalog = json!({"data":[
        {"id":"deepseek-v4.1-flash"},{"id":"mimo-v2.6-flash-free"},{"id":"longcat-2.5-preview-free"},
        {"id":"kimi-k3"},{"id":"glm-5.3"},{"id":"gemini-4-argon"},{"id":"gpt-5.3-codex"}
    ]});
    let endpoint = "https://opencode.ai/inference/v1";
    assert_eq!(
        filter_catalog(&config(endpoint, ApiProtocol::Openai), AgentKind::Opencode, catalog.clone()).unwrap()["data"],
        json!([{"id":"deepseek-v4.1-flash"},{"id":"mimo-v2.6-flash-free"},{"id":"longcat-2.5-preview-free"},
               {"id":"kimi-k3"},{"id":"glm-5.3"}])
    );
    assert_eq!(
        filter_catalog(&config(endpoint, ApiProtocol::Google), AgentKind::Gemini, catalog.clone()).unwrap()["data"],
        json!([{"id":"gemini-4-argon"}])
    );
    assert_eq!(
        filter_catalog(&config(endpoint, ApiProtocol::Openai), AgentKind::Codex, catalog).unwrap()["data"],
        json!([{"id":"gpt-5.3-codex"}])
    );
}
