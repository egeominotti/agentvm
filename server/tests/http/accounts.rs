use crate::helpers::{app, json_req, send};
use axum::http::StatusCode;
use serde_json::Value;

#[tokio::test]
async fn account_catalog_and_status_expose_only_connection_metadata() {
    let home = tempfile::tempdir().unwrap();
    let router = app(home.path());
    let (status, body) = send(router.clone(), json_req("GET", "/api/settings/accounts/chatgpt", Value::Null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "chatgpt");
    assert!(body["connected"].is_boolean());
    assert_eq!(body["can_login"], true);
    for key in ["access_token", "refresh_token", "credential", "token"] {
        assert!(body.get(key).is_none());
    }
    let (_, catalog) = send(router, json_req("GET", "/api/agents", Value::Null)).await;
    assert_eq!(catalog["accounts"].as_array().unwrap().len(), 5);
    let codex = catalog["agents"].as_array().unwrap().iter().find(|a| a["kind"] == "codex").unwrap();
    assert_eq!(codex["accounts"], serde_json::json!(["chatgpt"]));
    let claude = catalog["accounts"].as_array().unwrap().iter().find(|a| a["id"] == "claude").unwrap();
    assert_eq!(claude["can_import"], false);
}
