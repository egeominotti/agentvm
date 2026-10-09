//! This Mac at a glance (`/api/status`) and the disk agentvm uses (`/api/storage`).

use axum::Json;
use axum::extract::State;

use super::Ctx;
use super::dto::Status;
use crate::app::queries::{StorageUsage, cleanup_finished_jobs, storage_usage};

pub async fn status(State(ctx): Ctx) -> Json<Status> {
    // Usually from memory; when it is not, `security` is a process: never on the async workers.
    let keychain = ctx.clone();
    let token = tokio::task::spawn_blocking(move || keychain.keychain.token_status())
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    Json(Status {
        golden: ctx.config.golden().is_file(),
        token_hint: token.as_ref().err().cloned(),
        token: token.is_ok(),
        concurrency: ctx.scheduler.concurrency(),
        running: ctx.store.running_count(),
        host: ctx.settings.limits(),
        ram_committed_mb: ctx.store.committed_memory_mb(),
    })
}

pub async fn storage(State(ctx): Ctx) -> Json<StorageUsage> {
    Json(tokio::task::spawn_blocking(move || storage_usage(&ctx)).await.expect("storage scan"))
}

pub async fn cleanup(State(ctx): Ctx) -> Json<serde_json::Value> {
    let removed = tokio::task::spawn_blocking(move || cleanup_finished_jobs(&ctx)).await.expect("cleanup");
    Json(serde_json::json!({ "removed": removed }))
}
