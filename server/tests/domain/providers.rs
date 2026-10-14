//! Provider targets cannot hide credentials or change request routing through a query.
use agentvm::domain::providers::{ApiProtocol, Provider, ProviderConfig};
#[test]
fn provider_endpoint_refuses_embedded_secrets_and_non_http_urls() {
    for endpoint in [
        "",
        "file:///tmp/x",
        "https://user:secret@example.com",
        "https://example.com?key=secret",
        "https://example.com#path",
        "/relative",
    ] {
        let config = ProviderConfig { endpoint: endpoint.into(), protocol: ApiProtocol::Openai, requires_key: false };
        assert!(config.validate().is_err(), "{endpoint}");
    }
    for endpoint in ["https://example.org/v1", "http://127.0.0.1:11434/v1"] {
        assert!(
            ProviderConfig { endpoint: endpoint.into(), protocol: ApiProtocol::Openai, requires_key: false }
                .validate()
                .is_ok()
        );
    }
}

#[test]
fn subscription_keys_select_their_dedicated_provider_service() {
    let alibaba: Provider = serde_json::from_str("\"alibabacodingplan\"").unwrap();
    assert_eq!(alibaba.config().endpoint, "https://coding-intl.dashscope.aliyuncs.com/v1");
    assert_eq!(alibaba.config().protocol, ApiProtocol::Openai);
    assert!(alibaba.config().requires_key);
    let go: Provider = serde_json::from_str("\"opencodego\"").unwrap();
    assert_eq!(go.config().endpoint, "https://opencode.ai/inference/go/v1");
    let zen: Provider = serde_json::from_str("\"opencodezen\"").unwrap();
    assert_eq!(zen.config().endpoint, "https://opencode.ai/inference/v1");
    for provider in [alibaba, go, zen] {
        assert!(provider.config().validate().is_ok());
    }
}
