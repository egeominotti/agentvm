//! Snapshots leaving this Mac: export/import as a `.tar.zst` archive, and backups on S3
//! (`/api/backups`, configured at `/api/settings/s3`).

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

use super::Ctx;
use super::dto::S3Update;
use super::error::ApiError;
use super::snapshots::snapshot_id;
use crate::app::backups::{self, RemoteBackup};
use crate::domain::snapshot::SnapshotMeta;

/// Streams the archive; the temporary file is unlinked once open.
pub async fn export(State(ctx): Ctx, Path(sid): Path<String>) -> Result<Response, ApiError> {
    let sid = snapshot_id(&sid)?;
    let path = backups::export(&ctx, &sid).await?;
    let file = tokio::fs::File::open(&path).await.map_err(ApiError::internal)?;
    let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let _ = std::fs::remove_file(&path);
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(file));
    Ok((
        [
            ("content-type", "application/zstd".to_owned()),
            ("content-length", len.to_string()),
            ("content-disposition", format!("attachment; filename=\"{sid}.tar.zst\"")),
        ],
        body,
    )
        .into_response())
}

pub async fn import(State(ctx): Ctx, body: Body) -> Result<Json<SnapshotMeta>, ApiError> {
    // Deleted on every path out of here, including a broken upload.
    let upload = backups::temp_file(&ctx, "upload.tar.zst").map_err(ApiError::internal)?;
    {
        let mut file = tokio::fs::File::create(upload.path()).await.map_err(ApiError::internal)?;
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(ApiError::bad_request)?;
            file.write_all(&chunk).await.map_err(ApiError::internal)?;
        }
        file.flush().await.map_err(ApiError::internal)?;
    }
    Ok(Json(backups::import(&ctx, upload.path()).await?))
}

pub async fn backup(State(ctx): Ctx, Path(sid): Path<String>) -> Result<Json<RemoteBackup>, ApiError> {
    let sid = snapshot_id(&sid)?;
    Ok(Json(backups::backup(&ctx, &sid).await?))
}

pub async fn list(State(ctx): Ctx) -> Result<Json<Vec<RemoteBackup>>, ApiError> {
    Ok(Json(backups::list(&ctx).await?))
}

pub async fn restore(State(ctx): Ctx, Path(sid): Path<String>) -> Result<Json<SnapshotMeta>, ApiError> {
    let sid = snapshot_id(&sid)?;
    Ok(Json(backups::restore(&ctx, &sid).await?))
}

pub async fn delete(State(ctx): Ctx, Path(sid): Path<String>) -> Result<StatusCode, ApiError> {
    let sid = snapshot_id(&sid)?;
    backups::delete(&ctx, &sid).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_s3(State(ctx): Ctx) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "config": ctx.settings.get().s3,
        "secret_saved": ctx.keychain.read_s3_secret().is_ok(),
    }))
}

pub async fn put_s3(State(ctx): Ctx, Json(req): Json<S3Update>) -> Result<StatusCode, ApiError> {
    backups::configure_s3(&ctx, req.config, req.secret).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test_s3(State(ctx): Ctx) -> Result<StatusCode, ApiError> {
    backups::test_s3(&ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}
