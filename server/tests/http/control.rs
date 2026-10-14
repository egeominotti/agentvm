//! Structured control validates task lifecycle before reaching a guest.
use crate::helpers::{app, json_req, send};
use crate::telemetry::vm;
use axum::http::StatusCode;

#[tokio::test]
async fn structured_control_rejects_unknown_and_stopped_tasks() {
    let home = tempfile::tempdir().unwrap();
    let path = "/api/tasks/00000000/control/exec";
    let (status, _) = send(app(home.path()), json_req("POST", path, serde_json::json!({"argv":["true"]}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (ctx, id) = vm(home.path());
    let path = format!("/api/tasks/{id}/control/exec");
    let (status, body) =
        send(agentvm::http::router(ctx), json_req("POST", &path, serde_json::json!({"argv":["true"]}))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

#[tokio::test]
async fn structured_control_rejects_malformed_commands_before_guest_io() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = vm(home.path());
    let path = format!("/api/tasks/{id}/control/exec");
    let (status, body) =
        send(agentvm::http::router(ctx), json_req("POST", &path, serde_json::json!({"argv":[]}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
