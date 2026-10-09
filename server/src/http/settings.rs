//! Settings (`/api/settings`) and the Claude token kept in the keychain.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use super::Ctx;
use super::dto::{SettingsView, TokenUpdate};
use super::error::ApiError;
use crate::app::supervisor::AppCtx;
use crate::domain::settings::Settings;
use crate::secret::Secret;

fn view(ctx: &AppCtx) -> SettingsView {
    let settings = ctx.settings.get();
    let limits = ctx.settings.limits();
    SettingsView { recommended_max_vms: limits.recommended_vms(settings.memory_mb), settings, limits }
}

pub async fn get(State(ctx): Ctx) -> Json<SettingsView> {
    Json(view(&ctx))
}

pub async fn put(State(ctx): Ctx, Json(new): Json<Settings>) -> Result<Json<SettingsView>, ApiError> {
    // Two fsyncs: off the async workers.
    let saving = ctx.clone();
    tokio::task::spawn_blocking(move || saving.update_settings(new))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::bad_request)?;
    Ok(Json(view(&ctx)))
}

pub async fn put_token(State(ctx): Ctx, Json(req): Json<TokenUpdate>) -> Result<StatusCode, ApiError> {
    let secret = Secret::new(req.token.trim().to_owned());
    // `security` is a process: never on the async workers.
    tokio::task::spawn_blocking(move || ctx.keychain.write_token(&secret))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::bad_request)?;
    Ok(StatusCode::NO_CONTENT)
}
