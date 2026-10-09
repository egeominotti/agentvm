//! `/api/tasks/{id}/claude?cursor=<json>` and `/api/tasks/{id}/claude/usage`: Claude's
//! conversation and usage in a VM, kept after the VM is closed.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use super::Ctx;
use super::error::ApiError;
use super::tasks::find;
use crate::app::history::{Conversation, Cursor, conversation, usage};
use crate::domain::usage::UsageSample;

#[derive(Deserialize)]
pub struct Params {
    /// The `cursor` of the previous answer, as JSON (none: from the start).
    cursor: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Usage {
    samples: Vec<UsageSample>,
}

/// Off the async workers: both read files.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Option<T> + Send + 'static) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(ApiError::not_found)
}

pub async fn claude(
    State(ctx): Ctx,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> Result<Json<Conversation>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let cursor: Cursor = match p.cursor.as_deref() {
        Some(json) => {
            serde_json::from_str(json).map_err(|_| ApiError::bad_request("cursor is the one a previous answer gave"))?
        }
        None => Cursor::default(),
    };
    blocking(move || conversation(&ctx, &id, &cursor).ok()).await.map(Json)
}

pub async fn claude_usage(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<Usage>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    blocking(move || usage(&ctx, &id).ok()).await.map(|samples| Json(Usage { samples }))
}
