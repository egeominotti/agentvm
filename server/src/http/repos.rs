//! `GET /api/repos/check?path=…`: whether a VM can be launched on a folder, and why not.

use axum::Json;
use axum::extract::Query;
use serde::Deserialize;

use super::error::ApiError;
use crate::app::repos::{RepoCheck, check as check_repo};

#[derive(Deserialize)]
pub struct CheckQuery {
    path: String,
}

pub async fn check(Query(q): Query<CheckQuery>) -> Result<Json<RepoCheck>, ApiError> {
    // git runs a process: off the async threads.
    let answer = tokio::task::spawn_blocking(move || check_repo(&q.path)).await.map_err(ApiError::internal)?;
    Ok(Json(answer))
}
