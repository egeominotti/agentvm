//! Reads that involve external systems, exposed to HTTP without it knowing about the adapters.

use super::supervisor::AppCtx;
use crate::adapters::git::Git;
use crate::domain::ids::TaskId;
use crate::domain::task::TaskState;

#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("task not found")]
    NotFound,
    #[error("the task did not produce a branch")]
    NoBranch,
    #[error("{0}")]
    Git(String),
}

/// Diff between the base commit and the branch produced by the task.
pub async fn task_diff(ctx: &AppCtx, id: &TaskId) -> Result<String, DiffError> {
    let record = ctx.store.get(id).ok_or(DiffError::NotFound)?;
    if !matches!(record.state, TaskState::Done { .. }) {
        return Err(DiffError::NoBranch);
    }
    let branch = id.branch();
    tokio::task::spawn_blocking(move || Git::new(record.repo).diff(&record.base_sha, &branch))
        .await
        .map_err(|e| DiffError::Git(e.to_string()))?
        .map_err(|e| DiffError::Git(e.to_string()))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageUsage {
    pub jobs: usize,
    pub jobs_mb: u64,
    pub golden_mb: u64,
}

pub fn storage_usage(ctx: &AppCtx) -> StorageUsage {
    let jobs = std::fs::read_dir(ctx.config.jobs()).map_or(0, |d| d.flatten().count());
    StorageUsage {
        jobs,
        jobs_mb: crate::adapters::host::disk_usage(&ctx.config.jobs()) >> 20,
        golden_mb: crate::adapters::host::disk_usage(&ctx.config.home.join("golden")) >> 20,
    }
}

/// Deletes the folders of jobs that are not live; returns how many were removed.
pub fn cleanup_finished_jobs(ctx: &AppCtx) -> usize {
    let live: std::collections::HashSet<String> =
        ctx.store.list().into_iter().filter(|r| !r.state.is_terminal()).map(|r| r.id.to_string()).collect();
    let Ok(entries) = std::fs::read_dir(ctx.config.jobs()) else { return 0 };
    entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| !live.contains(name))
        .filter(|name| crate::adapters::jobdir::JobWorkspace::remove_job(&ctx.config.jobs(), name).is_ok())
        .count()
}
