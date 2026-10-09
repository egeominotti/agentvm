//! `/api/tasks/{id}/diagnostics`.

use std::sync::Arc;

use agentvm::adapters::keychain::Keychain;
use agentvm::app::supervisor::AppCtx;
use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::helpers::*;

/// A known VM's diagnostics: its summary, timeline and logs.
#[tokio::test]
async fn diagnostics_of_a_vm_have_their_shape() {
    let home = tempfile::tempdir().unwrap();
    let ctx = Arc::new(AppCtx::new(config(home.path()), Keychain::new(Some(home.path().join("none.keychain-db")))));
    std::fs::create_dir_all(home.path().join("repo/.git")).unwrap();
    let rec = agentvm::app::store::TaskRecord::new(
        agentvm::domain::ids::TaskId::generate(std::time::SystemTime::now(), &[1]),
        agentvm::domain::ids::RepoPath::new(home.path().join("repo")).unwrap(),
        None,
        agentvm::domain::ids::CommitSha::parse(&"a".repeat(40)).unwrap(),
        true,
    );
    let id = rec.id.clone();
    ctx.store.insert(rec);
    let req = Request::get(format!("/api/tasks/{id}/diagnostics"))
        .header("host", "127.0.0.1:7777")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(agentvm::http::router(ctx), req).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["summary"], "queued");
    assert_eq!(body["timeline"][0]["state"], "queued");
    assert!(body["logs"].is_array() && body["server_log"].is_array() && body["hint"].is_null(), "{body}");
}

#[tokio::test]
async fn diagnostics_of_an_unknown_vm_are_not_found() {
    let res = fetch("/api/tasks/0199c4b6-a2f2-7fff-bfff-ffffffffffff/diagnostics").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(fetch("/api/tasks/not-an-id/diagnostics").await.status(), StatusCode::NOT_FOUND);
}

/// The boot console is a terminal's output: it is shown without its control sequences.
#[tokio::test]
async fn the_boot_console_is_shown_as_plain_text() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = crate::telemetry::vm(home.path());
    let job = home.path().join("jobs").join(&id);
    std::fs::create_dir_all(&job).unwrap();
    std::fs::write(job.join("console.log"), "\x1b[6n\x1b[!p\x1b]104\x07\x1b[?7h\r\r\nDebian GNU/Linux 13\r\nagentvm login: ").unwrap();
    let req = Request::get(format!("/api/tasks/{id}/diagnostics")).header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    let (status, body) = send(agentvm::http::router(ctx), req).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let console = body["logs"].as_array().unwrap().iter().find(|l| l["file"] == "console.log").expect("no console log");
    assert_eq!(console["tail"], "\nDebian GNU/Linux 13\nagentvm login: ", "{console}");
}
