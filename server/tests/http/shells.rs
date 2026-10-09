//! Closing an extra shell of a VM: `DELETE /api/tasks/{id}/pty/{session}`.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::helpers::send;
use crate::telemetry::vm;

fn delete(id: &str, session: &str) -> Request<Body> {
    Request::delete(format!("/api/tasks/{id}/pty/{session}"))
        .header("host", "127.0.0.1:7777")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn only_an_extra_shell_can_be_closed() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = vm(home.path());
    for session in ["bash", "shell", "claude", "shell-10", "..%2Fclaude"] {
        let (status, body) = send(agentvm::http::router(ctx.clone()), delete(&id, session)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{session}: {body}");
    }
}

#[tokio::test]
async fn a_shell_of_a_machine_not_running_cannot_be_closed() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = vm(home.path());
    let (status, body) = send(agentvm::http::router(ctx), delete(&id, "shell-2")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}
