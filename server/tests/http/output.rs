//! Finite output pages use explicit cursors and bounded limits.
use crate::helpers::{json_req, send};
use crate::telemetry::vm;
use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn finite_output_exposes_normalized_events_and_rejects_invalid_limits() {
    use agentvm::app::events::StreamItem;
    use agentvm::domain::{agent_event::AgentEvent, ids::TaskId};
    let home = tempfile::tempdir().unwrap();
    let (ctx, id) = vm(home.path());
    ctx.store
        .log(&TaskId::parse(&id).unwrap())
        .unwrap()
        .push(StreamItem::Agent(AgentEvent::Text { text: "finite output".into() }));
    let router = agentvm::http::router(ctx);
    let path = format!("/api/tasks/{id}/output?cursor=0&limit=1");
    let (status, first) = send(router.clone(), json_req("GET", &path, json!(null))).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["items"][0]["seq"], 1);
    assert_eq!(first["more"], true);
    let path = format!("/api/tasks/{id}/output?cursor=1&limit=100");
    let (_, second) = send(router.clone(), json_req("GET", &path, json!(null))).await;
    assert_eq!(second["items"][0]["item"], json!({"type":"agent","data":{"kind":"text","text":"finite output"}}));
    assert_eq!(second["next_cursor"], 2);
    assert_eq!(second["dropped"], 0);
    assert_eq!(second["more"], false);
    for limit in ["0", "501", "-1", "oops"] {
        let path = format!("/api/tasks/{id}/output?limit={limit}");
        let (status, _) = send(router.clone(), json_req("GET", &path, json!(null))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
