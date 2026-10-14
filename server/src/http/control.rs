//! JSON wrappers for structured guest execution, file and terminal operations.
use super::{Ctx, error::ApiError};
use crate::app::control::{self, ControlError};
use crate::domain::ids::TaskId;
use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

async fn call(ctx: Ctx, id: String, op: &str, args: Value) -> Result<Json<Value>, ApiError> {
    let id = TaskId::parse(&id).ok_or_else(ApiError::not_found)?;
    let value = control::request(&ctx.0, &id, op, args).await.map_err(|e| {
        let code = match e {
            ControlError::NotFound => StatusCode::NOT_FOUND,
            ControlError::NotRunning => StatusCode::CONFLICT,
            ControlError::Invalid(_) => StatusCode::BAD_REQUEST,
            ControlError::Unreachable(_) => StatusCode::BAD_GATEWAY,
        };
        ApiError::new(code, e)
    })?;
    Ok(Json(value))
}

pub async fn start(ctx: Ctx, Path(id): Path<String>, Json(args): Json<Value>) -> Result<Json<Value>, ApiError> {
    call(ctx, id, "exec.start", args).await
}

pub async fn read(ctx: Ctx, Path((id, exec_id)): Path<(String, String)>) -> Result<Json<Value>, ApiError> {
    call(ctx, id, "exec.read", json!({"exec_id": exec_id})).await
}

pub async fn cancel(ctx: Ctx, Path((id, exec_id)): Path<(String, String)>) -> Result<Json<Value>, ApiError> {
    call(ctx, id, "exec.cancel", json!({"exec_id": exec_id})).await
}

async fn dispatch(ctx: Ctx, id: String, group: &str, mut args: Value) -> Result<Json<Value>, ApiError> {
    let op = args
        .as_object_mut()
        .and_then(|a| a.remove("op"))
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or_else(|| ApiError::bad_request("op is required"))?;
    call(ctx, id, &format!("{group}.{op}"), args).await
}

pub async fn files(ctx: Ctx, Path(id): Path<String>, Json(args): Json<Value>) -> Result<Json<Value>, ApiError> {
    dispatch(ctx, id, "files", args).await
}

pub async fn terminal(ctx: Ctx, Path(id): Path<String>, Json(args): Json<Value>) -> Result<Json<Value>, ApiError> {
    dispatch(ctx, id, "terminal", args).await
}
