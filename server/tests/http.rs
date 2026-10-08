//! Real HTTP router (tower oneshot): protection against DNS rebinding and cross-origin requests.

use std::sync::Arc;

use agentvm::adapters::keychain::Keychain;
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
        scripts_dir: "scripts".into(),
        min_free_mb: 0,
    };
    agentvm::http::router(Arc::new(AppCtx::new(config, Keychain::new(Some(home.join("none.keychain-db"))))))
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

async fn send(app: axum::Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

fn json_req(method: &str, path: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7777")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn settings_can_be_read_updated_and_persisted() {
    let tmp = tempfile::tempdir().unwrap();
    let (status, body) = send(
        app(tmp.path()),
        Request::get("/api/settings").header("host", "127.0.0.1:7777").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["limits"]["cpus"].as_u64().unwrap() >= 1);
    let mut s = body["settings"].clone();
    s["max_vms"] = 3.into();
    s["model"] = "opus".into();
    let (status, body) = send(app(tmp.path()), json_req("PUT", "/api/settings", s)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // A fresh server on the same home sees the saved settings.
    let (_, body) = send(
        app(tmp.path()),
        Request::get("/api/settings").header("host", "127.0.0.1:7777").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(body["settings"]["max_vms"], 3);
    assert_eq!(body["settings"]["model"], "opus");
}

#[tokio::test]
async fn invalid_settings_are_rejected_with_a_message() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, body) = send(
        app(tmp.path()),
        Request::get("/api/settings").header("host", "127.0.0.1:7777").body(Body::empty()).unwrap(),
    )
    .await;
    let mut s = body["settings"].clone();
    s["cpus"] = 999.into();
    let (status, body) = send(app(tmp.path()), json_req("PUT", "/api/settings", s)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("vCPU"), "{body}");
}

#[tokio::test]
async fn launch_rejects_resources_beyond_the_mac() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let git = |args: &[&str]| {
        assert!(std::process::Command::new("git").arg("-C").arg(&repo).args(args).status().unwrap().success())
    };
    git(&["init", "-q"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "i"]);
    let body = serde_json::json!({"repo_path": repo, "interactive": true, "cpus": 999, "memory_mb": 4096});
    let (status, body) = send(app(tmp.path()), json_req("POST", "/api/tasks", body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("vCPU"), "{body}");
}

/// The settings page sends every field it loaded; an S3 bucket set up since must survive that.
#[tokio::test]
async fn saving_settings_keeps_the_s3_bucket_configured_since() {
    let tmp = tempfile::tempdir().unwrap();
    let get_settings = || Request::get("/api/settings").header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    let (_, before) = send(app(tmp.path()), get_settings()).await;
    // Meanwhile the S3 form saved a bucket (written as `configure_s3` does).
    let mut with_s3 = before["settings"].clone();
    with_s3["s3"] = serde_json::json!({"endpoint": "http://127.0.0.1:9100", "region": "us-east-1", "bucket": "agentvm-backups",
        "prefix": "agentvm", "access_key": "k", "path_style": true});
    std::fs::write(tmp.path().join("settings.json"), with_s3.to_string()).unwrap();

    let (_, loaded) = send(app(tmp.path()), get_settings()).await;
    assert_eq!(
        loaded["settings"]["s3"]["bucket"], "agentvm-backups",
        "setup: the saved bucket was not loaded: {loaded}"
    );
    let mut stale = before["settings"].clone();
    stale["max_vms"] = 2.into();
    let (status, body) = send(app(tmp.path()), json_req("PUT", "/api/settings", stale)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, after) = send(app(tmp.path()), get_settings()).await;
    assert_eq!(after["settings"]["max_vms"], 2);
    assert_eq!(after["settings"]["s3"]["bucket"], "agentvm-backups", "the S3 settings were wiped");
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

async fn fetch(path: &str) -> axum::response::Response {
    let tmp = tempfile::tempdir().unwrap();
    let req = Request::get(path).header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    app(tmp.path()).oneshot(req).await.unwrap()
}

/// Every file of the dashboard is embedded and served with its type, without listing it in Rust.
#[tokio::test]
async fn every_dashboard_file_is_served_with_its_type() {
    let web = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http/web");
    let mut files = vec![];
    let mut dirs = vec![web.clone()];
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                dirs.push(path);
            } else if !e.file_name().to_string_lossy().starts_with('.') && path != web.join("index.html") {
                files.push(path.strip_prefix(&web).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    assert!(files.iter().any(|f| f.ends_with(".js")) && files.iter().any(|f| f.ends_with(".css")));
    for f in &files {
        let res = fetch(&format!("/assets/{f}")).await;
        assert_eq!(res.status(), StatusCode::OK, "{f}");
        let ty = res.headers()["content-type"].to_str().unwrap().to_owned();
        let want = match f.rsplit('.').next().unwrap() {
            "js" => "text/javascript",
            "css" => "text/css",
            "woff2" => "font/woff2",
            "svg" => "image/svg+xml",
            _ => continue,
        };
        assert!(ty.starts_with(want), "{f}: {ty}");
    }
    assert_eq!(fetch("/vendor/xterm.css").await.status(), StatusCode::OK);
    assert_eq!(fetch("/logo.svg").await.status(), StatusCode::OK);
    assert_eq!(fetch("/").await.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_or_escaping_asset_paths_are_not_found() {
    for path in ["/assets/nope.js", "/assets/js/../index.html", "/assets/../Cargo.toml", "/vendor/../../build.rs"] {
        assert_eq!(fetch(path).await.status(), StatusCode::NOT_FOUND, "{path}");
    }
}
