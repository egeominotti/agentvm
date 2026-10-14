//! Typed account routes return metadata only; credentials remain in the host broker.
use super::{Ctx, error::ApiError};
use crate::app::accounts;
use crate::domain::accounts::{AccountKind, AccountStatus, LoginStart, LoginStatus};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

pub async fn status(State(ctx): Ctx, Path(account): Path<AccountKind>) -> Result<Json<AccountStatus>, ApiError> {
    tokio::task::spawn_blocking(move || accounts::status(&ctx, account)).await.map(Json).map_err(ApiError::internal)
}
pub async fn import(State(ctx): Ctx, Path(account): Path<AccountKind>) -> Result<Json<AccountStatus>, ApiError> {
    tokio::task::spawn_blocking(move || accounts::import(&ctx, account))
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .map_err(ApiError::bad_request)
}
pub async fn delete(State(ctx): Ctx, Path(account): Path<AccountKind>) -> Result<StatusCode, ApiError> {
    tokio::task::spawn_blocking(move || accounts::delete(&ctx, account))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}
pub async fn login(State(ctx): Ctx, Path(account): Path<AccountKind>) -> Result<Json<LoginStart>, ApiError> {
    accounts::begin_login(&ctx, account).await.map(Json).map_err(ApiError::bad_request)
}
pub async fn poll(
    State(ctx): Ctx,
    Path((account, id)): Path<(AccountKind, String)>,
) -> Result<Json<LoginStatus>, ApiError> {
    accounts::login_status(&ctx, account, &id).map(Json).map_err(ApiError::bad_request)
}
