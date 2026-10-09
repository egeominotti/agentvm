//! Settings › Git access: `GET /api/settings/git` (the hosts with a token), `PUT` and `DELETE`
//! `/api/settings/git/{host}`. A token goes into the Keychain and never comes back out.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use super::Ctx;
use super::error::ApiError;
use crate::app::remote_repos;
use crate::secret::Secret;

#[derive(Deserialize)]
pub struct TokenBody {
    token: String,
}

pub async fn get(State(ctx): Ctx) -> Result<Json<Value>, ApiError> {
    // The Keychain runs a process per host: off the async threads.
    let hosts =
        tokio::task::spawn_blocking(move || remote_repos::token_hosts(&ctx)).await.map_err(ApiError::internal)?;
    Ok(Json(json!({ "hosts": hosts })))
}

pub async fn put(
    State(ctx): Ctx,
    Path(host): Path<String>,
    Json(body): Json<TokenBody>,
) -> Result<StatusCode, ApiError> {
    let token = Secret::new(body.token);
    tokio::task::spawn_blocking(move || remote_repos::save_token(&ctx, &host, token))
        .await
        .map_err(ApiError::internal)??;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete(State(ctx): Ctx, Path(host): Path<String>) -> Result<StatusCode, ApiError> {
    tokio::task::spawn_blocking(move || remote_repos::remove_token(&ctx, &host))
        .await
        .map_err(ApiError::internal)??;
    Ok(StatusCode::NO_CONTENT)
}
