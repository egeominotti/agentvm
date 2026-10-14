//! Catalog and provider settings. Credentials never appear in responses.
use super::Ctx;
use super::error::ApiError;
use crate::app::agents::{self, AgentCatalog};
use crate::domain::providers::{Provider, ProviderConfig};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ProviderUpdate {
    pub config: ProviderConfig,
    #[serde(default)]
    pub token: Option<String>,
}

pub async fn catalog(State(ctx): Ctx) -> Result<Json<AgentCatalog>, ApiError> {
    tokio::task::spawn_blocking(move || agents::catalog(&ctx)).await.map(Json).map_err(ApiError::internal)
}
pub async fn update(
    State(ctx): Ctx,
    Path(provider): Path<Provider>,
    Json(req): Json<ProviderUpdate>,
) -> Result<StatusCode, ApiError> {
    tokio::task::spawn_blocking(move || agents::update_provider(&ctx, provider, req.config, req.token))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::bad_request)?;
    Ok(StatusCode::OK)
}
pub async fn delete(State(ctx): Ctx, Path(provider): Path<Provider>) -> Result<StatusCode, ApiError> {
    tokio::task::spawn_blocking(move || agents::delete_provider_credential(&ctx, provider))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}
