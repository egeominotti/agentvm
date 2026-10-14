use agentvm::domain::account_proxy::{account_target, chatgpt_body};
use agentvm::domain::accounts::AccountKind;
use agentvm::domain::providers::{ApiProtocol, ProviderConfig};
use serde_json::json;

fn config(endpoint: &str) -> ProviderConfig {
    ProviderConfig { endpoint: endpoint.into(), protocol: ApiProtocol::Openai, requires_key: true }
}

#[test]
fn account_relay_keeps_credentials_at_the_verified_native_backend() {
    let codex = config("https://chatgpt.com/backend-api/codex");
    assert_eq!(
        account_target(AccountKind::Chatgpt, &codex, "/backend-api/codex/responses").unwrap().as_str(),
        "https://chatgpt.com/backend-api/codex/responses"
    );
    assert!(account_target(AccountKind::Chatgpt, &config("https://api.openai.com/v1"), "/v1/responses").is_err());
    assert!(
        account_target(
            AccountKind::Chatgpt,
            &config("https://evil.test/backend-api/codex"),
            "/backend-api/codex/responses"
        )
        .is_err()
    );
    assert!(account_target(AccountKind::Chatgpt, &codex, "/backend-api/codex/auth/token").is_err());
}

#[test]
fn copilot_uses_regional_backends_and_keeps_session_credentials_on_the_host() {
    let regional = config("https://api.business.githubcopilot.com");
    for path in ["/models", "/models/gpt-current/policy", "/responses", "/chat/completions", "/v1/messages"] {
        assert!(account_target(AccountKind::Copilot, &regional, path).is_ok(), "{path}");
    }
    for path in ["/models/session", "/models/session/intent", "/copilot_internal/v2/token", "//evil.test/models"] {
        assert!(account_target(AccountKind::Copilot, &regional, path).is_err(), "{path}");
    }
    for endpoint in
        ["http://api.githubcopilot.com", "https://githubcopilot.com.evil.test", "https://api.githubcopilot.com:444"]
    {
        assert!(account_target(AccountKind::Copilot, &config(endpoint), "/models").is_err(), "{endpoint}");
    }
}

#[test]
fn google_account_uses_code_assist_instead_of_api_key_authentication() {
    let google = config("https://cloudcode-pa.googleapis.com");
    for path in
        ["/v1internal:loadCodeAssist", "/v1internal:streamGenerateContent?alt=sse", "/v1internal/operations/example"]
    {
        assert!(account_target(AccountKind::Google, &google, path).is_ok(), "{path}");
    }
    assert!(account_target(AccountKind::Google, &google, "/oauth2/token").is_err());
    assert!(
        account_target(AccountKind::Google, &config("https://generativelanguage.googleapis.com"), "/v1beta/models")
            .is_err()
    );
}

#[test]
fn kilo_gateway_does_not_expose_device_login_or_account_configuration() {
    let kilo = config("https://api.kilo.ai/api/openrouter");
    assert!(account_target(AccountKind::Kilo, &kilo, "/api/openrouter/chat/completions").is_ok());
    assert!(account_target(AccountKind::Kilo, &kilo, "/api/openrouter/models").is_ok());
    assert!(account_target(AccountKind::Kilo, &kilo, "/api/device-auth/codes/example").is_err());
}

#[test]
fn chatgpt_requests_lift_instructions_and_preserve_literal_tools_and_input() {
    let body = json!({"store":true,"stream":false,"instructions":"existing",
                      "input":[{"role":"system","content":[{"type":"input_text","text":"system"}]},
                               {"role":"developer","content":"developer"},
                               {"role":"user","content":"literal $(touch should-not-run)"}],
                      "tools":[{"type":"function","name":"read_file"}],"reasoning":{"effort":"low"},
                      "max_output_tokens":42,"temperature":0.5});
    let normalized: serde_json::Value =
        serde_json::from_slice(&chatgpt_body(&serde_json::to_vec(&body).unwrap()).unwrap()).unwrap();
    assert_eq!(normalized["instructions"], "existing\n\nsystem\n\ndeveloper");
    assert_eq!(normalized["input"], json!([{"role":"user","content":"literal $(touch should-not-run)"}]));
    assert_eq!(normalized["tools"], json!([{"type":"function","name":"read_file"}]));
    assert_eq!(normalized["reasoning"], json!({"effort":"low"}));
    assert_eq!(normalized["store"], false);
    assert_eq!(normalized["stream"], true);
    assert!(normalized.get("max_output_tokens").is_none());
    assert!(normalized.get("temperature").is_none());
}

#[test]
fn chatgpt_invalid_request_errors_do_not_echo_the_request() {
    let error = chatgpt_body(b"private input is not JSON").unwrap_err();
    assert!(!error.contains("private input"));
    assert!(chatgpt_body(b"[]").is_err());
}

#[test]
fn account_headers_keep_native_client_identity_and_replace_all_guest_credentials() {
    use agentvm::domain::account_proxy::account_request_header;
    for name in [
        "content-type",
        "user-agent",
        "copilot-integration-id",
        "x-github-api-version",
        "x-interaction-id",
        "x-goog-user-project",
        "originator",
        "session_id",
        "x-client-request-id",
        "x-codex-beta-features",
    ] {
        assert!(account_request_header(name), "{name}");
    }
    for name in ["authorization", "x-api-key", "x-goog-api-key", "copilot-session-token", "cookie"] {
        assert!(!account_request_header(name), "{name}");
    }
}

#[test]
fn copilot_catalog_uses_original_oauth_and_the_regional_auth_scheme() {
    use agentvm::domain::account_proxy::{account_authorization, uses_catalog_credential};
    for path in ["/models", "/models?client=agentvm", "/models/gpt-current/policy"] {
        assert!(uses_catalog_credential(AccountKind::Copilot, "GET", path));
    }
    for (method, path) in [
        ("POST", "/models"),
        ("GET", "/models/session"),
        ("GET", "/models/session/intent"),
        ("POST", "/responses"),
        ("GET", "/chat/completions"),
    ] {
        assert!(!uses_catalog_credential(AccountKind::Copilot, method, path));
    }
    assert!(!uses_catalog_credential(AccountKind::Chatgpt, "GET", "/models"));
    for endpoint in ["https://api.githubcopilot.com", "https://api.individual.githubcopilot.com"] {
        assert_eq!(
            account_authorization(AccountKind::Copilot, &config(endpoint), "gho-native", true),
            "Bearer gho-native"
        );
    }
    for endpoint in ["https://api.business.githubcopilot.com", "https://api.enterprise.githubcopilot.com"] {
        let cfg = config(endpoint);
        assert_eq!(account_authorization(AccountKind::Copilot, &cfg, "gho-native", true), "token gho-native");
        assert_eq!(
            account_authorization(AccountKind::Copilot, &cfg, "github_pat_native", true),
            "Bearer github_pat_native"
        );
        assert_eq!(account_authorization(AccountKind::Copilot, &cfg, "minted-session", false), "Bearer minted-session");
    }
}
