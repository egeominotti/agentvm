//! The agent catalog exposes supported choices without exposing credentials.
use crate::helpers::{app, send};
use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn catalog_lists_all_agents_without_secret_values() {
    let home = tempfile::tempdir().unwrap();
    let req = Request::get("/api/agents").header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    let (status, body) = send(app(home.path()), req).await;
    assert_eq!(status, StatusCode::OK);
    let kinds: Vec<_> = body["agents"].as_array().unwrap().iter().map(|a| a["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["claude", "codex", "gemini", "opencode", "qwen", "copilot", "kilo", "aider", "goose", "custom"]);
    let providers: Vec<_> = body["providers"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(
        providers,
        [
            "openai",
            "anthropic",
            "google",
            "openrouter",
            "alibabacodingplan",
            "opencodego",
            "opencodezen",
            "local",
            "custom"
        ]
    );
    assert!(body["claude_token"].is_boolean());
    assert!(!body.to_string().contains("sk-"));
}

#[tokio::test]
async fn provider_configuration_is_persisted_without_secret_fields() {
    let home = tempfile::tempdir().unwrap();
    let config = serde_json::json!({"endpoint":"http://127.0.0.1:8080/v1","protocol":"openai","requires_key":false});
    let (status, _) = send(
        app(home.path()),
        crate::helpers::json_req("PUT", "/api/settings/providers/local", serde_json::json!({"config":config})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.path().join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["providers"]["local"], config);
    assert!(settings.get("token").is_none());
}

#[tokio::test]
async fn provider_catalog_only_reports_presence_and_delete_removes_the_saved_key() {
    use agentvm::adapters::keychain::Keychain;
    use agentvm::app::context::AppCtx;
    use agentvm::domain::providers::Provider;
    use agentvm::secret::Secret;
    use std::process::Command;
    use std::sync::Arc;
    let home = tempfile::tempdir().unwrap();
    let kc_path = home.path().join("catalog.keychain-db");
    assert!(Command::new("security").args(["create-keychain", "-p", "x"]).arg(&kc_path).status().unwrap().success());
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
        }
    }
    let _cleanup = Cleanup(kc_path.clone());
    let keychain = Keychain::new(Some(kc_path.clone()));
    let router = agentvm::http::router(Arc::new(AppCtx::new(crate::helpers::config(home.path()), keychain.clone())));
    let config = serde_json::json!({"endpoint":"https://example.org/v1","protocol":"openai","requires_key":true});
    let token = "provider_catalog_secret_123";
    let (status, body) = send(
        router.clone(),
        crate::helpers::json_req(
            "PUT",
            "/api/settings/providers/custom",
            serde_json::json!({"config":config,"token":token}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.to_string().contains(token));
    let (status, body) =
        send(router.clone(), crate::helpers::json_req("GET", "/api/agents", serde_json::Value::Null)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.to_string().contains(token));
    let custom = body["providers"].as_array().unwrap().iter().find(|p| p["id"] == "custom").unwrap();
    assert_eq!(custom["has_key"], true);
    assert_eq!(custom["config"], config);
    assert!(!std::fs::read_to_string(home.path().join("settings.json")).unwrap().contains(token));
    let (status, _) =
        send(router, crate::helpers::json_req("DELETE", "/api/settings/providers/custom", serde_json::Value::Null))
            .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(keychain.read_provider_key(Provider::Custom).is_none());
    assert!(keychain.write_provider_key(Provider::Custom, &Secret::new("bad token".into())).is_err());
}

#[tokio::test]
async fn custom_headless_validation_precedes_missing_golden_image() {
    let (home, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    assert!(
        std::process::Command::new("git").arg("-C").arg(repo.path()).args(["init", "-q"]).status().unwrap().success()
    );
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "one"])
            .status()
            .unwrap()
            .success()
    );
    let request = serde_json::json!({"repo_path":repo.path(),"prompt":"task","agent":{"kind":"custom","custom":{"name":"Example","interactive":"example {prompt}"}}});
    let (status, body) = send(app(home.path()), crate::helpers::json_req("POST", "/api/tasks", request)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("headless"));
}
