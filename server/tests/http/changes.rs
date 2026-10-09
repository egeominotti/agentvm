//! `/api/changes`: a push each time the task list changes, so a background tab (whose timers the
//! browser slows to once a minute) still learns at once that Claude is waiting.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures::StreamExt;
use tower::ServiceExt;

use crate::telemetry::vm;

async fn next_event(body: &mut axum::body::BodyDataStream) -> Option<String> {
    let chunk = tokio::time::timeout(Duration::from_secs(3), body.next()).await.ok()??.ok()?;
    Some(String::from_utf8_lossy(&chunk).into_owned())
}

#[tokio::test]
async fn a_change_to_any_task_is_pushed_at_once() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = vm(home.path());
    let req = Request::get("/api/changes").header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    let res = agentvm::http::router(ctx.clone()).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "text/event-stream");
    let mut body = res.into_body().into_data_stream();
    // One right away: a client that reconnects re-reads the list in case it missed something.
    let first = next_event(&mut body).await.expect("no first event");
    // A browser drops an event without data: each one needs its data line.
    assert!(first.contains("event: changed\n") && first.contains("\ndata:"), "{first:?}");

    let id = agentvm::domain::ids::TaskId::parse(&id).unwrap();
    ctx.store.set_activity(&id, Some("waiting".into()));
    assert!(next_event(&mut body).await.expect("the change was not pushed").contains("changed"));

    ctx.store.remove(&id);
    assert!(next_event(&mut body).await.expect("the removal was not pushed").contains("changed"));
}

/// Telemetry arrives every second per VM: it must not wake every dashboard each time.
#[tokio::test]
async fn telemetry_alone_is_not_pushed() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = vm(home.path());
    let req = Request::get("/api/changes").header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    let mut body = agentvm::http::router(ctx.clone()).oneshot(req).await.unwrap().into_body().into_data_stream();
    next_event(&mut body).await.expect("no first event");
    let id = agentvm::domain::ids::TaskId::parse(&id).unwrap();
    let metrics = serde_json::json!({"uptime_s": 1, "cpus": 2, "cpu_pct": 1.0, "load1": 0.0, "mem_used_mb": 1,
        "mem_total_mb": 2, "disk_used_mb": 1, "disk_total_mb": 2, "net_rx_bps": 0, "net_tx_bps": 0, "procs": 1, "top": []});
    ctx.store.record_metrics(&id, serde_json::from_value(metrics).unwrap());
    ctx.store.set_activity(&id, None);
    let quiet = tokio::time::timeout(Duration::from_millis(400), body.next()).await;
    assert!(quiet.is_err(), "pushed without a change: {quiet:?}");
}
