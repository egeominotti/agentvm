use agentvm::adapters::accounts::{parse_session, read_reference, validate_copilot_endpoint};
use agentvm::domain::accounts::AccountKind;
use serde_json::json;

#[tokio::test]
#[ignore = "downloads public installed-client metadata from the pinned official HTTPS source"]
async fn google_public_metadata_fetch_is_valid_and_cached_only_in_host_memory() {
    use agentvm::adapters::accounts::login::google_client;
    let first = google_client::load().await.unwrap();
    first.validate().unwrap();
    let second = google_client::load().await.unwrap();
    assert!(std::ptr::eq(first, second));
}

#[test]
fn kilo_native_api_login_reads_the_key_without_changing_the_file() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("auth.json");
    let text = r#"{"kilo":{"type":"api","key":"kilo-host-token"}}"#;
    std::fs::write(&path, text).unwrap();
    let session = read_reference(AccountKind::Kilo, &path).unwrap();
    assert_eq!(session.access_token, "kilo-host-token");
    assert!(session.refresh_token.is_none());
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    assert!(parse_session(AccountKind::Google, &json!({"type":"api","key":"foreign"})).is_err());
}

#[test]
fn kilo_native_organization_login_keeps_identity_without_a_refresh_token() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("auth.json");
    let text =
        r#"{"kilo":{"type":"oauth","access":"kilo-org-token","refresh":"","expires":0,"accountId":"org-selected"}}"#;
    std::fs::write(&path, text).unwrap();
    let session = read_reference(AccountKind::Kilo, &path).unwrap();
    assert_eq!(session.access_token, "kilo-org-token");
    assert_eq!(session.account_id.as_deref(), Some("org-selected"));
    assert_eq!(session.expires_at, 0);
    assert!(session.refresh_token.is_none());
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    assert!(parse_session(AccountKind::Google, &json!({"access_token":"google","refresh_token":""})).is_err());
}

#[test]
fn native_sessions_keep_their_account_identity_and_expiry() {
    let google = parse_session(
        AccountKind::Google,
        &json!({"access_token":"google-access","refresh_token":"stable-refresh","expiry_date":2000000000000_u64}),
    )
    .unwrap();
    assert_eq!(google.expires_at, 2000000000);
    assert_eq!(google.refresh_token.as_deref(), Some("stable-refresh"));
    let kilo = parse_session(
        AccountKind::Kilo,
        &json!({"kilo":{"type":"oauth","access":"opaque","refresh":"opaque","expires":2000000000000_u64}}),
    )
    .unwrap();
    assert_eq!(kilo.access_token, "opaque");
    assert!(
        parse_session(AccountKind::Chatgpt, &json!({"auth_mode":"apikey","OPENAI_API_KEY":"not-account"})).is_err()
    );
    assert!(parse_session(AccountKind::Google, &json!({"access_token":"bad\nheader"})).is_err());
}

#[test]
fn chatgpt_cli_import_does_not_adopt_api_mode_or_direct_siwc_tokens() {
    use base64::Engine;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(r#"{"exp":2000000000,"scope":"chatgpt.tokens.use.direct"}"#);
    let token = format!("header.{payload}.signature");
    assert!(
        parse_session(AccountKind::Chatgpt, &json!({"tokens":{"access_token":token,"account_id":"workspace"}}))
            .is_err()
    );
    assert!(
        parse_session(
            AccountKind::Chatgpt,
            &json!({"auth_mode":"apikey","tokens":{"access_token":"old-oauth","account_id":"workspace"}})
        )
        .is_err()
    );
}

#[test]
fn copilot_import_rejects_classic_pat_and_foreign_provider_tokens() {
    for token in ["ghp_classic", "sk-openai-foreign"] {
        assert!(parse_session(AccountKind::Copilot, &json!({"token":token})).is_err());
    }
    assert!(parse_session(AccountKind::Copilot, &json!({"token":"github_pat_supported"})).is_ok());
}

#[test]
fn referenced_file_is_reloaded_without_modification() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("oauth.json");
    std::fs::write(&path, r#"{"access_token":"first","expiry_date":2000000000000}"#).unwrap();
    assert_eq!(read_reference(AccountKind::Google, &path).unwrap().access_token, "first");
    let changed = r#"{"access_token":"second","expiry_date":2000000000000}"#;
    std::fs::write(&path, changed).unwrap();
    assert_eq!(read_reference(AccountKind::Google, &path).unwrap().access_token, "second");
    assert_eq!(std::fs::read_to_string(path).unwrap(), changed);
}

#[test]
fn copilot_plaintext_fallback_uses_the_selected_user_only() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("config.json");
    let text = r#"// Copilot config
{"lastLoggedInUser":{"host":"https://github.com","login":"selected"},"authTokens":{"https://github.com:selected:github":{"token":"gho_selected"},"https://github.com:other:github":{"token":"gho_other"}}}"#;
    std::fs::write(&path, text).unwrap();
    assert_eq!(read_reference(AccountKind::Copilot, &path).unwrap().access_token, "gho_selected");
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
}

#[test]
fn copilot_native_keychain_query_is_bound_to_the_selected_profile() {
    use agentvm::adapters::accounts::{Reference, copilot_references};
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("config.json");
    std::fs::write(&path,r#"{"last_logged_in_user":{"host":"https://github.com","login":"selected"},"copilot_tokens":{"https://github.com:selected":"gho_selected"}}"#).unwrap();
    assert_eq!(read_reference(AccountKind::Copilot, &path).unwrap().access_token, "gho_selected");
    let references = copilot_references(&path).unwrap();
    let Reference::Keychain { service, account } = &references[1] else { panic!("selected profile reference missing") };
    assert_eq!(service, "copilot-cli");
    assert_eq!(account.as_deref(), Some("https://github.com:selected"));
}

#[test]
fn copilot_regional_origin_cannot_redirect_credentials_elsewhere() {
    for valid in ["https://api.githubcopilot.com", "https://api.business.githubcopilot.com"] {
        assert!(validate_copilot_endpoint(valid).is_ok());
    }
    for bad in [
        "http://api.githubcopilot.com",
        "https://api.githubcopilot.com.evil.org",
        "https://evil.org",
        "https://user@api.githubcopilot.com",
        "https://api.githubcopilot.com/other",
        "https://api.githubcopilot.com?token=bad",
        "https://api.example.ghe.com",
    ] {
        assert!(validate_copilot_endpoint(bad).is_err(), "accepted {bad}");
    }
}
