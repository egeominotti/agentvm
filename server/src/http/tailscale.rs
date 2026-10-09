//! Tailscale: the auth key (`GET`/`PUT`/`DELETE /api/settings/tailscale`, never sent back out) and
//! joining or leaving the tailnet from a running VM's page (`POST`/`DELETE /api/tasks/{id}/tailscale`).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use super::Ctx;
use super::error::ApiError;
use super::tasks::find;
use crate::app::tailscale::{self, TailscaleError};
use crate::secret::Secret;

#[derive(Deserialize)]
pub struct KeyBody {
    key: String,
}

pub async fn get_key(State(ctx): Ctx) -> Result<Json<Value>, ApiError> {
    // `security` is a process: off the async workers.
    let saved = tokio::task::spawn_blocking(move || tailscale::key_saved(&ctx)).await.map_err(ApiError::internal)?;
    Ok(Json(json!({ "key_saved": saved })))
}

pub async fn put_key(State(ctx): Ctx, Json(body): Json<KeyBody>) -> Result<StatusCode, ApiError> {
    let key = Secret::new(body.key);
    tokio::task::spawn_blocking(move || tailscale::save_key(&ctx, key))
        .await
        .map_err(ApiError::internal)?
        .map_err(tailscale_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_key(State(ctx): Ctx) -> Result<StatusCode, ApiError> {
    tokio::task::spawn_blocking(move || tailscale::remove_key(&ctx))
        .await
        .map_err(ApiError::internal)?
        .map_err(tailscale_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn join(State(ctx): Ctx, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    tailscale::join(&ctx, &id).await.map_err(tailscale_error)?;
    Ok(StatusCode::ACCEPTED)
}

pub async fn leave(State(ctx): Ctx, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    tailscale::leave(&ctx, &id).await.map_err(tailscale_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn tailscale_error(e: TailscaleError) -> ApiError {
    let code = match e {
        TailscaleError::NotFound => StatusCode::NOT_FOUND,
        TailscaleError::NotRunning => StatusCode::CONFLICT,
        TailscaleError::NoKey | TailscaleError::InvalidKey => StatusCode::BAD_REQUEST,
        TailscaleError::Timeout => StatusCode::GATEWAY_TIMEOUT,
        TailscaleError::Keychain(_) | TailscaleError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiError::new(code, e)
}
