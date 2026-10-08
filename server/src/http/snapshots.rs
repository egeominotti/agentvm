//! Snapshots on this Mac (`/api/snapshots`): take one of a running terminal, list, restore into a
//! new VM, delete, and each machine's schedule of automatic snapshots.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use super::Ctx;
use super::dto::{AutoSnapshotInterval, Created, SnapshotRequest, TaskDto};
use super::error::ApiError;
use super::tasks::{current, find};
use crate::app::snapshots;
use crate::domain::snapshot::{SnapshotId, SnapshotMeta};

/// The snapshot behind an `{sid}` path segment; 404 when it is not even an id.
pub fn snapshot_id(s: &str) -> Result<SnapshotId, ApiError> {
    SnapshotId::parse(s).ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "snapshot not found"))
}

pub async fn take(
    State(ctx): Ctx,
    Path(id): Path<String>,
    body: Option<Json<SnapshotRequest>>,
) -> Result<Json<SnapshotMeta>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let name = body.and_then(|Json(b)| b.name);
    Ok(Json(snapshots::take_snapshot(&ctx, &id, name).await?))
}

pub async fn list(State(ctx): Ctx) -> Json<Vec<SnapshotMeta>> {
    Json(tokio::task::spawn_blocking(move || snapshots::list(&ctx)).await.expect("list snapshots"))
}

pub async fn restore(State(ctx): Ctx, Path(sid): Path<String>) -> Result<(StatusCode, Json<Created>), ApiError> {
    let sid = snapshot_id(&sid)?;
    let id = snapshots::restore(&ctx, &sid)?;
    Ok((StatusCode::CREATED, Json(Created { id: id.to_string() })))
}

pub async fn delete(State(ctx): Ctx, Path(sid): Path<String>) -> Result<StatusCode, ApiError> {
    let sid = snapshot_id(&sid)?;
    snapshots::delete(&ctx, &sid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `{"every_min": 15}` for this machine only, `{"every_min": null}` to follow the settings.
pub async fn set_auto(
    State(ctx): Ctx,
    Path(id): Path<String>,
    Json(req): Json<AutoSnapshotInterval>,
) -> Result<Json<TaskDto>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    snapshots::set_interval(&ctx, &id, req.every_min)?;
    current(&ctx, &id)
}
