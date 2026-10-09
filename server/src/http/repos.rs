//! `GET /api/repos/check?path=…`: whether a VM can be launched on a folder or a link, and why not;
//! `POST /api/tasks/{id}/push`: a VM's branch sent to its repository's origin.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use super::Ctx;
use super::error::ApiError;
use super::tasks::find;
use crate::app::remote_repos::{self, PushedBranch};
use crate::app::repos::{RepoCheck, check as check_repo};

#[derive(Deserialize)]
pub struct CheckQuery {
    path: String,
}

pub async fn check(State(ctx): Ctx, Query(q): Query<CheckQuery>) -> Result<Json<RepoCheck>, ApiError> {
    // git runs a process: off the async threads.
    let home = ctx.config.home.clone();
    let answer = tokio::task::spawn_blocking(move || check_repo(&home, &q.path)).await.map_err(ApiError::internal)?;
    Ok(Json(answer))
}

pub async fn push(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<PushedBranch>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    // git over the network: off the async threads.
    let pushed =
        tokio::task::spawn_blocking(move || remote_repos::push(&ctx, &id)).await.map_err(ApiError::internal)??;
    Ok(Json(pushed))
}
