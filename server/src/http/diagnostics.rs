//! `/api/tasks/{id}/diagnostics`: why a VM failed, what to do, and the evidence.

use axum::Json;
use axum::extract::{Path, State};

use super::Ctx;
use super::error::ApiError;
use super::tasks::find;
use crate::app::diagnostics::{Diagnostics, of};

pub async fn diagnostics(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<Diagnostics>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    // Off the async workers: it reads log files.
    tokio::task::spawn_blocking(move || of(&ctx, &id))
        .await
        .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map(Json)
        .map_err(|_| ApiError::not_found())
}
