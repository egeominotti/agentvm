//! Finite event pagination for dashboards and external clients.
use super::{Ctx, error::ApiError};
use crate::app::{
    events::OutputPage,
    output::{self, OutputError},
};
use crate::domain::ids::TaskId;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct OutputQuery {
    #[serde(default)]
    cursor: u64,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_limit() -> usize {
    100
}

pub async fn output(
    State(ctx): Ctx,
    Path(id): Path<String>,
    Query(query): Query<OutputQuery>,
) -> Result<Json<OutputPage>, ApiError> {
    let id = TaskId::parse(&id).ok_or_else(ApiError::not_found)?;
    tokio::task::spawn_blocking(move || output::page(&ctx, &id, query.cursor, query.limit))
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .map_err(|error| {
            ApiError::new(
                match error {
                    OutputError::NotFound => StatusCode::NOT_FOUND,
                    OutputError::Limit => StatusCode::BAD_REQUEST,
                },
                error,
            )
        })
}
