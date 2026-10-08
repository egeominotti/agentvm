//! `/api/tasks/{id}/telemetry?range=5m|1h|all`: a VM's telemetry for the dashboard's charts.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use super::Ctx;
use super::error::ApiError;
use super::tasks::find;
use crate::app::telemetry::{Range, series};
use crate::domain::telemetry::TelemetrySample;

#[derive(Deserialize)]
pub struct Params {
    range: Option<String>,
}

#[derive(Serialize)]
pub struct Series {
    range: String,
    points: Vec<TelemetrySample>,
}

pub async fn telemetry(
    State(ctx): Ctx,
    Path(id): Path<String>,
    Query(p): Query<Params>,
) -> Result<Json<Series>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let name = p.range.unwrap_or_else(|| "5m".into());
    let range = Range::parse(&name).ok_or_else(|| ApiError::bad_request("range is 5m, 1h or all"))?;
    // Off the async workers: it may read the VM's history file.
    let points = tokio::task::spawn_blocking(move || series(&ctx, &id, range))
        .await
        .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map_err(|_| ApiError::not_found())?;
    Ok(Json(Series { range: name, points }))
}
