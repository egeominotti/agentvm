//! The real router on a temporary home, and requests to it.

use std::sync::Arc;

use agentvm::adapters::keychain::Keychain;
use agentvm::app::supervisor::AppCtx;
use agentvm::config::Config;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

pub(crate) fn app(home: &std::path::Path) -> axum::Router {
    agentvm::http::router(Arc::new(AppCtx::new(config(home), Keychain::new(Some(home.join("none.keychain-db"))))))
}

pub(crate) fn config(home: &std::path::Path) -> Config {
    Config {
        home: home.to_path_buf(),
        port: 7777,
        concurrency: 1,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 60,
        vm_helper: "agentvm-vm".into(),
        scripts_dir: "scripts".into(),
        min_free_mb: 0,
    }
}

pub(crate) async fn status_of(req: Request<Body>) -> StatusCode {
    let tmp = tempfile::tempdir().unwrap();
    app(tmp.path()).oneshot(req).await.unwrap().status()
}

pub(crate) fn get(host: &str) -> Request<Body> {
    Request::get("/api/status").header("host", host).body(Body::empty()).unwrap()
}

pub(crate) async fn send(app: axum::Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

pub(crate) fn json_req(method: &str, path: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7777")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub(crate) async fn fetch(path: &str) -> axum::response::Response {
    let tmp = tempfile::tempdir().unwrap();
    let req = Request::get(path).header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    app(tmp.path()).oneshot(req).await.unwrap()
}
