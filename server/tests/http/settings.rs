//! Settings read, saved and validated over HTTP; launches checked against the Mac.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::helpers::*;

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
