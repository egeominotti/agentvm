//! An interactive VM's session: save its work to the repo, close it, drop files into it.

use axum::Json;
use axum::extract::{Path, Request, State};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

use super::Ctx;
use super::dto::{Saved, TaskDto};
use super::error::ApiError;
use super::tasks::{current, find};
use crate::app::session;

/// Largest file that can be dropped on a terminal.
const UPLOAD_MAX: u64 = 2 << 30;

pub async fn save(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<Saved>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let saved = session::save(&ctx, &id).await?;
    Ok(Json(Saved { commits: saved.commits, branch: saved.branch }))
}

pub async fn close(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<TaskDto>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    session::close(&ctx, &id).await?;
    current(&ctx, &id)
}

/// A file dropped on a terminal: `x-file-name` (URL-encoded) + the raw bytes. Returns its path in the VM.
pub async fn upload(
    State(ctx): Ctx,
    Path(id): Path<String>,
    req: Request,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let name = req
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .ok_or_else(|| ApiError::bad_request("missing x-file-name"))?;
    let up = session::start_upload(&ctx, &id, &name)?;
    let guest_path = up.guest_path;
    let new_file = up.file;
    let mut file = tokio::fs::File::from_std(new_file.file.try_clone().map_err(ApiError::internal)?);
    let mut body = req.into_body().into_data_stream();
    let mut written = 0u64;
    let result: Result<(), String> = async {
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            written += chunk.len() as u64;
            if written > UPLOAD_MAX {
                return Err("files over 2 GB cannot be dropped on a terminal".into());
            }
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())
    }
    .await;
    drop(file);
    if let Err(e) = result {
        // Through the folder's handle: the guest may have made `uploads` a link meanwhile.
        new_file.discard();
        return Err(ApiError::bad_request(e));
    }
    Ok(Json(serde_json::json!({ "path": guest_path, "bytes": written })))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(b) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
