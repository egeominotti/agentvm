//! Router HTTP reale (tower oneshot): protezione da DNS rebinding e richieste cross-origin.

use std::sync::Arc;

use agentvm::adapters::keychain::Keychain;
use agentvm::app::scheduler::Scheduler;
use agentvm::app::store::Store;
use agentvm::app::supervisor::AppCtx;
use agentvm::config::Config;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn app(home: &std::path::Path) -> axum::Router {
    let config = Config {
        home: home.to_path_buf(),
        port: 7777,
        concurrency: 1,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 60,
        vm_helper: "agentvm-vm".into(),
    };
    agentvm::http::router(Arc::new(AppCtx {
        scheduler: Scheduler::new(1),
        store: Store::new(),
        keychain: Keychain::new(Some(home.join("none.keychain-db"))),
        config,
    }))
}

async fn status_of(req: Request<Body>) -> StatusCode {
    let tmp = tempfile::tempdir().unwrap();
    app(tmp.path()).oneshot(req).await.unwrap().status()
}

fn get(host: &str) -> Request<Body> {
    Request::get("/api/status").header("host", host).body(Body::empty()).unwrap()
}

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
