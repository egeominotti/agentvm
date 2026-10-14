//! Actual public model discovery and the native Coding Plan metadata adapter, without inference.
use agentvm::adapters::agent_proxy::ProviderRelay;
use agentvm::domain::agents::AgentKind;
use agentvm::domain::providers::{ApiProtocol, ProviderConfig};
use agentvm::secret::Secret;
use axum::body::Body;
use axum::http::Request;
use hyper_util::rt::TokioIo;

async fn catalog(path: &std::path::Path, endpoint: &str, protocol: ApiProtocol, kind: AgentKind) -> serde_json::Value {
    let relay = ProviderRelay::start_for_agent(
        path,
        ProviderConfig { endpoint: endpoint.into(), protocol, requires_key: false },
        Secret::new(String::new()),
        false,
        kind,
        "catalog-verification".into(),
    )
    .unwrap();
    let socket = tokio::net::UnixStream::connect(path).await.unwrap();
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(socket)).await.unwrap();
    tokio::spawn(connection);
    let prefix = url::Url::parse(endpoint).unwrap().path().trim_end_matches('/').to_owned();
    let prefix =
        if protocol == ApiProtocol::Anthropic && !prefix.ends_with("/v1") { format!("{prefix}/v1") } else { prefix };
    let response = sender
        .send_request(Request::get(format!("{prefix}/models")).header("host", "guest").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(response.status().is_success());
    let bytes = axum::body::to_bytes(Body::new(response.into_body()), 8 << 20).await.unwrap();
    let catalog = serde_json::from_slice(&bytes).unwrap();
    drop(relay);
    catalog
}

#[tokio::test]
async fn coding_plan_catalog_is_available_through_the_real_guest_relay() {
    let tmp = tempfile::tempdir().unwrap();
    let data = catalog(
        &tmp.path().join("plan.sock"),
        "https://coding-intl.dashscope.aliyuncs.com/v1",
        ApiProtocol::Openai,
        AgentKind::Qwen,
    )
    .await;
    assert!(data["data"].as_array().unwrap().iter().any(|model| model["id"] == "qwen3.7-plus"));
}

#[tokio::test]
async fn coding_plan_anthropic_catalog_is_available_through_the_real_guest_relay() {
    let tmp = tempfile::tempdir().unwrap();
    for (region, host) in
        ["coding.dashscope.aliyuncs.com", "coding-intl.dashscope.aliyuncs.com"].into_iter().enumerate()
    {
        for (version, path) in ["/apps/anthropic", "/apps/anthropic/v1"].into_iter().enumerate() {
            let endpoint = format!("https://{host}{path}");
            let config =
                ProviderConfig { endpoint: endpoint.clone(), protocol: ApiProtocol::Anthropic, requires_key: false };
            // A missing local catalog would send this regression to the subscription service.
            assert!(agentvm::domain::provider_models::preset_catalog(&config).is_some());
            let socket = tmp.path().join(format!("anthropic-{region}-{version}.sock"));
            let data = catalog(&socket, &endpoint, ApiProtocol::Anthropic, AgentKind::Claude).await;
            assert_eq!(data["data"][0]["id"], "qwen3.7-plus");
            assert_eq!(data["data"][0]["type"], "model");
            assert_eq!(data["has_more"], false);
            assert!(data["data"][0].get("created_at").is_none());
        }
    }
}

#[tokio::test]
#[ignore = "requires public provider network; model discovery only, no inference quota"]
async fn opencode_go_public_catalog_matches_responses_in_the_real_relay() {
    let tmp = tempfile::tempdir().unwrap();
    let data = catalog(
        &tmp.path().join("go.sock"),
        "https://opencode.ai/inference/go/v1",
        ApiProtocol::Openai,
        AgentKind::Codex,
    )
    .await;
    let models = data["data"].as_array().unwrap();
    assert!(!models.is_empty());
    assert!(models.iter().all(|model| matches!(
        model["id"].as_str(),
        Some(
            "gpt-6-luna"
                | "gpt-5.6-luna"
                | "grok-4.7"
                | "grok-4.6"
                | "muse-spark-1.3-contributor"
                | "muse-spark-1.2-contributor"
        )
    )));
}
