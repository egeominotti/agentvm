//! The public task API allows automatic model selection without requiring a model input.
use crate::helpers::{config, json_req, send};
use agentvm::adapters::keychain::Keychain;
use agentvm::app::context::AppCtx;
use agentvm::domain::agents::{AgentKind, AgentSelection};
use agentvm::domain::providers::Provider;
use agentvm::domain::settings::Model;
use axum::http::StatusCode;
use std::process::Command;
use std::sync::Arc;

#[tokio::test]
async fn api_omitted_model_allows_automatic_selection() {
    let (home, repo) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for args in [
        vec!["init", "-q"],
        vec!["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "one"],
    ] {
        assert!(Command::new("git").arg("-C").arg(repo.path()).args(args).status().unwrap().success());
    }
    let ctx = Arc::new(AppCtx::new(config(home.path()), Keychain::new(Some(home.path().join("none.keychain-db")))));
    let mut settings = ctx.settings.get();
    settings.agent =
        AgentSelection { kind: AgentKind::Opencode, provider: Some(Provider::Local), ..Default::default() };
    settings.model = Model::default_choice();
    ctx.update_settings(settings).unwrap();
    let router = agentvm::http::router(ctx);
    let (status, body) =
        send(router, json_req("POST", "/api/tasks", serde_json::json!({"repo_path":repo.path(),"prompt":"task"})))
            .await;
    // The real missing-image check ends this request before any VM can start. Reaching it
    // proves that automatic model selection passed validation at the API boundary.
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(body["error"].as_str().unwrap().contains("golden image missing"));
}
