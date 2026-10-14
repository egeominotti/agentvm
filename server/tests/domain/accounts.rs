use agentvm::domain::agents::AgentSelection;
use serde_json::json;

#[test]
fn google_public_metadata_parser_reads_only_exact_constant_declarations() {
    use agentvm::domain::accounts::parse_google_oauth_source;
    let source = "// OAUTH_CLIENT_ID = 'comment';\nconst OAUTH_CLIENT_ID =\n  'fixture-client';\nconst OAUTH_CLIENT_SECRET = \"fixture-public-value\";\n";
    let config = parse_google_oauth_source(source).unwrap();
    assert_eq!(config.client_id, "fixture-client");
    assert_eq!(config.client_secret, "fixture-public-value");
    assert!(config.validate().is_err());
    for bad in [
        "const OAUTH_CLIENT_ID_FAKE = 'fixture';\nconst OAUTH_CLIENT_SECRET = 'fixture';",
        "const OAUTH_CLIENT_ID = 'fixture';",
        "const OAUTH_CLIENT_ID = 'fixture';\nconst OAUTH_CLIENT_ID = 'duplicate';\nconst OAUTH_CLIENT_SECRET = 'fixture';",
        "const OAUTH_CLIENT_ID = 'fixture' + 'suffix';\nconst OAUTH_CLIENT_SECRET = 'fixture';",
        "const OAUTH_CLIENT_ID = 'fixture\\nvalue';\nconst OAUTH_CLIENT_SECRET = 'fixture';",
        "/*\nconst OAUTH_CLIENT_ID = 'fixture-client';\nconst OAUTH_CLIENT_SECRET = 'fixture-public-value';\n*/",
    ] {
        assert!(parse_google_oauth_source(bad).is_err());
    }
    assert!(parse_google_oauth_source(&"x".repeat(131073)).is_err());
}

#[test]
fn kilo_fresh_login_uses_only_an_eligible_selected_or_required_organization() {
    use agentvm::domain::accounts::kilo_organization;
    let mut profile = json!({"user":{"email":"member@example.org"},"organizations":[{"id":"org-first","name":"First"},{"id":"org-selected","name":"Selected"}],"selectedOrganizationId":"org-selected","hasPersonalAccount":true});
    assert_eq!(kilo_organization(&profile).unwrap().as_deref(), Some("org-selected"));
    profile["selectedOrganizationId"] = json!("foreign-org");
    assert!(kilo_organization(&profile).unwrap().is_none());
    profile["hasPersonalAccount"] = json!(false);
    assert_eq!(kilo_organization(&profile).unwrap().as_deref(), Some("org-first"));
    profile["organizations"] = json!([]);
    assert!(kilo_organization(&profile).is_err());
    assert!(
        kilo_organization(&json!({"user":{"email":"personal@example.org"},"hasPersonalAccount":true}))
            .unwrap()
            .is_none()
    );
    assert!(kilo_organization(&json!({"organizations":[{"id":"bad\nheader"}],"hasPersonalAccount":false})).is_err());
}

#[test]
fn only_copilot_keeps_its_original_oauth_for_the_model_catalog() {
    use agentvm::domain::accounts::AccountKind;
    let source = "gho_original_host_login";
    assert_eq!(AccountKind::Copilot.catalog_credential(source).unwrap().expose(), source);
    for account in [AccountKind::Claude, AccountKind::Chatgpt, AccountKind::Google, AccountKind::Kilo] {
        assert!(account.catalog_credential(source).is_none());
    }
}

#[test]
fn authentication_choice_survives_json_roundtrip() {
    let agent: AgentSelection =
        serde_json::from_value(json!({"kind":"codex","auth":"account","account_kind":"chatgpt"})).unwrap();
    let encoded = serde_json::to_value(agent).unwrap();
    assert_eq!(encoded["auth"], "account");
    assert_eq!(encoded["account_kind"], "chatgpt");
}

#[test]
fn unsupported_or_ambiguous_accounts_are_rejected() {
    for value in [
        json!({"kind":"aider","auth":"account","account_kind":"chatgpt"}),
        json!({"kind":"codex","auth":"api","account_kind":"chatgpt"}),
        json!({"kind":"codex","auth":"account","provider":"openai"}),
        json!({"kind":"codex","account_kind":"copilot"}),
    ] {
        let agent: AgentSelection = serde_json::from_value(value).unwrap();
        assert!(agent.validate(None, true).is_err(), "accepted {agent:?}");
    }
}

#[test]
fn oauth_callback_requires_the_expected_state_and_callback_path() {
    use agentvm::domain::accounts::callback_code;
    assert_eq!(
        callback_code("/oauth2callback?state=expected&code=issued-code", "expected").unwrap(),
        Some("issued-code".into())
    );
    for path in [
        "/other?state=expected&code=x",
        "/oauth2callback?state=wrong&code=x",
        "/oauth2callback?state=wrong&state=expected&code=x",
    ] {
        assert_eq!(callback_code(path, "expected").unwrap(), None);
    }
    assert!(callback_code("/oauth2callback?state=expected&error=access_denied", "expected").is_err());
}
