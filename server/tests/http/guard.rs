//! Protection against DNS rebinding, cross-origin requests, framing and proxied names.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use tower::ServiceExt;

use crate::helpers::*;

#[tokio::test]
async fn accepts_loopback_hosts() {
    assert_eq!(status_of(get("127.0.0.1:7777")).await, StatusCode::OK);
    assert_eq!(status_of(get("localhost:7777")).await, StatusCode::OK);
}

#[tokio::test]
async fn rejects_foreign_host_header() {
    assert_eq!(status_of(get("evil.example:7777")).await, StatusCode::FORBIDDEN);
    assert_eq!(status_of(get("127.0.0.1:8080")).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn rejects_cross_origin_post() {
    let req = Request::post("/api/tasks")
        .header("host", "127.0.0.1:7777")
        .header("origin", "http://evil.example")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"repo_path":"/tmp","prompt":"x"}"#))
        .unwrap();
    assert_eq!(status_of(req).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn same_origin_post_reaches_the_handler() {
    let req = Request::post("/api/tasks")
        .header("host", "127.0.0.1:7777")
        .header("origin", "http://127.0.0.1:7777")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"repo_path":"/nonexistent","prompt":"x"}"#))
        .unwrap();
    assert_eq!(status_of(req).await, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_cross_origin_websocket_to_a_terminal() {
    let req = Request::get("/api/tasks/20261008-000000-abcd/pty?session=shell")
        .header("host", "127.0.0.1:7777")
        .header("origin", "http://evil.example")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap();
    assert_eq!(status_of(req).await, StatusCode::FORBIDDEN);
}

/// Another site must not be able to show the dashboard in a frame and trick clicks on it.
#[tokio::test]
async fn the_dashboard_refuses_to_be_framed() {
    let tmp = tempfile::tempdir().unwrap();
    for path in ["/", "/api/status"] {
        let res = app(tmp.path())
            .oneshot(Request::get(path).header("host", "127.0.0.1:7777").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.headers().get("x-frame-options").map(|v| v.to_str().unwrap()), Some("DENY"), "{path}");
        let csp =
            res.headers().get("content-security-policy").map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default();
        assert!(csp.contains("frame-ancestors 'none'"), "{path}: {csp}");
    }
}

/// `<port>.<vm>.localhost` belongs to a VM's service: it must never reach the dashboard's API.
#[tokio::test]
async fn proxied_names_never_reach_the_api() {
    let tmp = tempfile::tempdir().unwrap();
    let req =
        Request::get("/api/status").header("host", "3000.nothing-0000.localhost:7777").body(Body::empty()).unwrap();
    let res = app(tmp.path()).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("no running machine is called nothing-0000") && !text.contains("golden"), "{text}");
}
