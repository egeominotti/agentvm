//! `/api/tasks/{id}/telemetry` and the freshness of what the task list says.

use std::sync::Arc;

use agentvm::adapters::keychain::Keychain;
use agentvm::app::supervisor::AppCtx;
use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::helpers::*;

pub(crate) fn vm(home: &std::path::Path) -> (Arc<AppCtx>, String) {
    let ctx = Arc::new(AppCtx::new(config(home), Keychain::new(Some(home.join("none.keychain-db")))));
    std::fs::create_dir_all(home.join("repo/.git")).unwrap();
    let rec = agentvm::app::store::TaskRecord::new(
        agentvm::domain::ids::TaskId::generate(std::time::SystemTime::now(), &[2]),
        agentvm::domain::ids::RepoPath::new(home.join("repo")).unwrap(),
        None,
        agentvm::domain::ids::CommitSha::parse(&"a".repeat(40)).unwrap(),
        true,
    );
    let id = rec.id.to_string();
    ctx.store.insert(rec);
    (ctx, id)
}

fn get_at(path: String) -> Request<Body> {
    Request::get(path).header("host", "127.0.0.1:7777").body(Body::empty()).unwrap()
}

#[tokio::test]
async fn telemetry_is_served_per_range() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = vm(home.path());
    for range in ["5m", "1h", "all"] {
        let (status, body) =
            send(agentvm::http::router(ctx.clone()), get_at(format!("/api/tasks/{id}/telemetry?range={range}"))).await;
        assert_eq!(status, StatusCode::OK, "{range}: {body}");
        assert!(body["points"].is_array() && body["range"] == range, "{body}");
    }
    let (status, _) = send(agentvm::http::router(ctx), get_at(format!("/api/tasks/{id}/telemetry?range=week"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// The task list says how old its numbers are, and how much memory the VM may keep.
#[tokio::test]
async fn tasks_say_how_fresh_their_numbers_are() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = vm(home.path());
    let (_, body) = send(agentvm::http::router(ctx), get_at(format!("/api/tasks/{id}"))).await;
    assert!(body.get("metrics_age_s").is_some() && body["metrics_age_s"].is_null(), "no sample yet: {body}");
    assert!(body.get("memory_limit_mb").is_some(), "{body}");
}
