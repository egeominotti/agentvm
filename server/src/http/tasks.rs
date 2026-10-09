//! Tasks (`/api/tasks`): launch, list, inspect, stop, forget, and the diff of their work.

use crate::app::remote_repos;
use crate::domain::branches::start_ref;
use crate::domain::git_remote::GitRemote;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use super::Ctx;
use super::dto::{CreateTask, Created, TaskDto};
use super::error::ApiError;
use crate::app::queries::{remove_task, task_diff};
use crate::app::store::TaskRecord;
use crate::app::supervisor::{AppCtx, NewTask, submit};
use crate::domain::ids::TaskId;
use crate::domain::settings::ClaudeVersion;

/// The task behind an `{id}` path segment; 404 when it is unknown or not even an id.
pub fn find(ctx: &AppCtx, id: &str) -> Result<(TaskId, TaskRecord), ApiError> {
    let id = TaskId::parse(id).ok_or_else(ApiError::not_found)?;
    let record = ctx.store.get(&id).ok_or_else(ApiError::not_found)?;
    Ok((id, record))
}

/// The task as the API shows it, after a change.
pub fn current(ctx: &AppCtx, id: &TaskId) -> Result<Json<TaskDto>, ApiError> {
    Ok(Json(find(ctx, id.as_str())?.1.into()))
}

pub async fn create(State(ctx): Ctx, Json(req): Json<CreateTask>) -> Result<(StatusCode, Json<Created>), ApiError> {
    let claude_version = match req.claude_version.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => Some(ClaudeVersion::parse(v).map_err(ApiError::bad_request)?),
        None => None,
    };
    // A link: cloned (or fetched) first, then launched from that clone like any repository.
    let from_link = GitRemote::looks_like_link(&req.repo_path);
    let base_ref = match start_ref(req.branch.as_deref(), from_link).map_err(ApiError::bad_request)? {
        Some(r) => Some(r),
        None => req.base_ref.clone(),
    };
    let repo = if from_link {
        let (ctx, link) = (ctx.clone(), req.repo_path.clone());
        let opened = tokio::task::spawn_blocking(move || remote_repos::open(&ctx, &link))
            .await
            .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e))??;
        opened.path.display().to_string()
    } else {
        req.repo_path.clone()
    };
    // Off the async workers: it reads the repository and the Keychain.
    let id = tokio::task::spawn_blocking(move || {
        let new = NewTask {
            repo: &repo,
            prompt: req.prompt,
            base_ref: base_ref.as_deref(),
            interactive: req.interactive,
            model: req.model,
            claude_version,
            restore_from: None,
            cpus: req.cpus,
            memory_mb: req.memory_mb,
            label: None,
            tailscale: req.tailscale,
        };
        submit(&ctx, new)
    })
    .await
    .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e))??;
    Ok((StatusCode::CREATED, Json(Created { id: id.to_string() })))
}

pub async fn list(State(ctx): Ctx) -> Json<Vec<TaskDto>> {
    Json(ctx.store.list().into_iter().map(TaskDto::from).collect())
}

pub async fn detail(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<TaskDto>, ApiError> {
    Ok(Json(find(&ctx, &id)?.1.into()))
}

pub async fn diff(State(ctx): Ctx, Path(id): Path<String>) -> Result<impl IntoResponse, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let text = task_diff(&ctx, &id).await?;
    Ok(([("content-type", "text/plain; charset=utf-8")], text))
}

pub async fn stop(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<TaskDto>, ApiError> {
    let (id, record) = find(&ctx, &id)?;
    if record.state.is_terminal() {
        return Err(ApiError::conflict("the task has already finished"));
    }
    ctx.store.request_stop(&id).map_err(ApiError::conflict)?;
    current(&ctx, &id)
}

pub async fn remove(State(ctx): Ctx, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    remove_task(&ctx, &id)?;
    Ok(StatusCode::NO_CONTENT)
}
