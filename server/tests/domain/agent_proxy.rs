use agentvm::domain::agent_proxy::{StreamRedactor, authentication, guest_endpoint, target_url};
use agentvm::domain::providers::ApiProtocol;

#[test]
fn relay_keeps_the_configured_origin_and_base_path() {
    let endpoint = "https://openrouter.ai/api/v1";
    assert_eq!(guest_endpoint(endpoint).unwrap(), "http://127.0.0.1:5001/api/v1");
    assert_eq!(
        target_url(endpoint, "/api/v1/chat/completions?alt=sse").unwrap().as_str(),
        "https://openrouter.ai/api/v1/chat/completions?alt=sse"
    );
    assert!(target_url(endpoint, "/elsewhere").is_err());
    assert!(target_url(endpoint, "/api/v10/models").is_err());
}

#[test]
fn relay_rejects_absolute_urls_and_encoded_traversal() {
    for path in [
        "https://evil.test/v1",
        "//evil.test/v1",
        "/v1/../admin",
        "/v1/%2e%2E/admin",
        "/v1/a%2fb",
        "/v1/a%5Cb",
        "/v1/x#fragment",
        "/v1\\x",
    ] {
        assert!(target_url("https://api.openai.com/v1", path).is_err(), "{path}");
    }
}

#[test]
fn relay_strips_guest_query_credentials_and_preserves_stream_parameters() {
    let url = target_url(
        "https://generativelanguage.googleapis.com",
        "/v1beta/models/m:streamGenerateContent?key=dummy&alt=sse&access_token=bad",
    )
    .unwrap();
    assert_eq!(url.as_str(), "https://generativelanguage.googleapis.com/v1beta/models/m:streamGenerateContent?alt=sse");
}

#[test]
fn relay_replaces_credentials_for_each_protocol() {
    assert_eq!(
        authentication(ApiProtocol::Openai, "host-key", false),
        vec![("authorization".into(), "Bearer host-key".into())]
    );
    assert_eq!(
        authentication(ApiProtocol::Anthropic, "host-key", false),
        vec![("x-api-key".into(), "host-key".into())]
    );
    assert_eq!(
        authentication(ApiProtocol::Anthropic, "subscription-key", true),
        vec![("authorization".into(), "Bearer subscription-key".into())]
    );
    assert_eq!(
        authentication(ApiProtocol::Google, "host-key", false),
        vec![("x-goog-api-key".into(), "host-key".into())]
    );
    assert!(authentication(ApiProtocol::Openai, "", false).is_empty());
}

#[test]
fn streaming_redaction_handles_credentials_split_across_chunks() {
    let mut redact = StreamRedactor::new(b"private-key".to_vec());
    let mut output = redact.push(b"data: priv");
    output.extend(redact.push(b"ate-key says hello\n"));
    output.extend(redact.finish());
    assert_eq!(String::from_utf8(output).unwrap(), "data: [REDACTED] says hello\n");
}

#[test]
fn streaming_redaction_does_not_delay_unrelated_sse_messages() {
    let mut redact = StreamRedactor::new(b"a-long-private-key".to_vec());
    assert_eq!(redact.push(b"data: hello\n\n"), b"data: hello\n\n");
    assert!(redact.finish().is_empty());
}

#[test]
fn native_model_defaults_require_the_clis_original_provider_endpoint() {
    use agentvm::domain::agent_proxy::native_model_defaults;
    use agentvm::domain::agents::AgentKind;
    use agentvm::domain::providers::Provider;
    for (kind, provider) in [
        (AgentKind::Codex, Provider::Openai),
        (AgentKind::Gemini, Provider::Google),
        (AgentKind::Claude, Provider::Anthropic),
    ] {
        let mut config = provider.config();
        assert!(native_model_defaults(kind, &config));
        config.endpoint = "http://127.0.0.1:11434/v1".into();
        assert!(!native_model_defaults(kind, &config));
    }
    assert!(!native_model_defaults(AgentKind::Qwen, &Provider::Openai.config()));
}
