//! The golden image every VM boots from (`/api/golden`), and the Claude Code releases it can carry.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use super::Ctx;
use super::error::ApiError;
use crate::app::golden::GoldenStatus;
use crate::app::supervisor::ClaudeReleases;

pub async fn status(State(ctx): Ctx) -> Json<GoldenStatus> {
    Json(ctx.golden.status())
}

pub async fn rebuild(State(ctx): Ctx) -> Result<Json<GoldenStatus>, ApiError> {
    let version = ctx.settings.get().claude_version;
    ctx.golden.rebuild(version.as_str()).map_err(ApiError::conflict)?;
    Ok(Json(ctx.golden.status()))
}

pub async fn claude_versions(State(ctx): Ctx) -> Result<Json<ClaudeReleases>, ApiError> {
    ctx.claude_releases().await.map(Json).map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, e))
}
