//! `/api/tasks/{id}/claude` and `/api/tasks/{id}/claude/usage`.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::helpers::*;

#[tokio::test]
async fn claudes_history_has_its_shape() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = crate::telemetry::vm(home.path());
    let get = |path: String| Request::get(path).header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    let (status, body) = send(agentvm::http::router(ctx.clone()), get(format!("/api/tasks/{id}/claude"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["entries"].is_array() && body["cursor"].is_object() && body["more"] == false, "{body}");
    let bad = send(agentvm::http::router(ctx.clone()), get(format!("/api/tasks/{id}/claude?cursor=nope"))).await;
    assert_eq!(bad.0, StatusCode::BAD_REQUEST);
    let (status, body) = send(agentvm::http::router(ctx), get(format!("/api/tasks/{id}/claude/usage"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["samples"].is_array(), "{body}");
}
