//! Route dell'API (spec §5).

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::{Stream, StreamExt};

use super::dto::{ApiError, CreateTask, Created, Status, TaskDto};
use crate::adapters::git::Git;
use crate::app::events::StreamItem;
use crate::app::store::TaskRecord;
use crate::app::supervisor::{AppCtx, SubmitError, submit};
use crate::domain::ids::TaskId;
use crate::domain::task::TaskState;

type Ctx = State<Arc<AppCtx>>;

pub fn router(ctx: Arc<AppCtx>) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/api/status", get(status))
        .route("/api/tasks", get(list).post(create))
        .route("/api/tasks/{id}", get(detail))
        .route("/api/tasks/{id}/events", get(events))
        .route("/api/tasks/{id}/diff", get(diff))
        .route("/api/tasks/{id}/stop", post(stop))
        .with_state(ctx)
}

async fn dashboard() -> Html<&'static str> {
    Html(include_str!("web/index.html"))
}

async fn status(State(ctx): Ctx) -> Json<Status> {
    let token = ctx.keychain.read_token();
    Json(Status {
        golden: ctx.config.golden().is_file(),
        token_hint: token.as_ref().err().map(ToString::to_string),
        token: token.is_ok(),
        concurrency: ctx.scheduler.concurrency(),
        running: ctx.scheduler.running(),
    })
}

async fn create(State(ctx): Ctx, Json(req): Json<CreateTask>) -> Result<(StatusCode, Json<Created>), ApiError> {
    let id = submit(&ctx, &req.repo_path, req.prompt, req.base_ref.as_deref()).map_err(|e| {
        let code = match e {
            SubmitError::NoGolden(_) | SubmitError::Token(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::BAD_REQUEST,
        };
        ApiError(code, e.to_string())
    })?;
    Ok((StatusCode::CREATED, Json(Created { id: id.to_string() })))
}

async fn list(State(ctx): Ctx) -> Json<Vec<TaskDto>> {
    Json(ctx.store.list().into_iter().map(TaskDto::from).collect())
}

fn find(ctx: &AppCtx, id: &str) -> Result<(TaskId, TaskRecord), ApiError> {
    let id = TaskId::parse(id).ok_or_else(ApiError::not_found)?;
    let record = ctx.store.get(&id).ok_or_else(ApiError::not_found)?;
    Ok((id, record))
}

async fn detail(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<TaskDto>, ApiError> {
    Ok(Json(find(&ctx, &id)?.1.into()))
}

async fn events(
    State(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let log = ctx.store.log(&id).ok_or_else(ApiError::not_found)?;
    let stream = log.stream().map(|(seq, item)| {
        let event = Event::default().id(seq.to_string());
        Ok(match item {
            StreamItem::State(s) => event.event("state").json_data(s),
            StreamItem::Agent(a) => event.event("agent").json_data(a),
        }
        .expect("eventi sempre serializzabili"))
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

async fn diff(State(ctx): Ctx, Path(id): Path<String>) -> Result<impl IntoResponse, ApiError> {
    let (id, record) = find(&ctx, &id)?;
    if !matches!(record.state, TaskState::Done { .. }) {
        return Err(ApiError(StatusCode::CONFLICT, "il task non ha prodotto un branch".into()));
    }
    let text = tokio::task::spawn_blocking(move || Git::new(record.repo).diff(&record.base_sha, &id.branch()))
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(([("content-type", "text/plain; charset=utf-8")], text))
}

async fn stop(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<TaskDto>, ApiError> {
    let (id, record) = find(&ctx, &id)?;
    if record.state.is_terminal() {
        return Err(ApiError(StatusCode::CONFLICT, "il task è già terminato".into()));
    }
    ctx.store.request_stop(&id).map_err(|e| ApiError(StatusCode::CONFLICT, e.to_string()))?;
    Ok(Json(find(&ctx, id.as_str())?.1.into()))
}
