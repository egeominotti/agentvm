//! Reads that involve external systems, exposed to HTTP without it knowing about the adapters.

use super::context::AppCtx;
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
    /// Logs and results of jobs (VM disks excluded).
    pub jobs_mb: u64,
    /// Disks of the VMs that are running now.
    pub vm_disks_mb: u64,
    pub golden_mb: u64,
    pub snapshots_mb: u64,
}

pub fn storage_usage(ctx: &AppCtx) -> StorageUsage {
    use crate::adapters::host::disk_usage;
    let jobs = ctx.config.jobs();
    let dirs: Vec<std::path::PathBuf> =
        std::fs::read_dir(&jobs).map_or(Vec::new(), |d| d.flatten().map(|e| e.path()).collect());
    let disks: u64 = dirs.iter().map(|d| disk_usage(&d.join("disk.raw")) + disk_usage(&d.join("efivars"))).sum();
    StorageUsage {
        jobs: dirs.len(),
        jobs_mb: disk_usage(&jobs).saturating_sub(disks) >> 20,
        vm_disks_mb: disks >> 20,
        golden_mb: disk_usage(&ctx.config.home.join("golden")) >> 20,
        snapshots_mb: disk_usage(&ctx.config.home.join("snapshots")) >> 20,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoveError {
    #[error("task not found")]
    NotFound,
    #[error("the VM is still running: close or stop it first")]
    StillRunning,
}

/// Forgets a finished task and deletes its job folder (logs included).
pub fn remove_task(ctx: &AppCtx, id: &TaskId) -> Result<(), RemoveError> {
    let record = ctx.store.get(id).ok_or(RemoveError::NotFound)?;
    if !record.state.is_terminal() {
        return Err(RemoveError::StillRunning);
    }
    ctx.store.remove(id);
    let _ = crate::adapters::jobdir::JobWorkspace::remove_job(&ctx.config.jobs(), id.as_str());
    Ok(())
}

/// Deletes the folders of tasks known to be finished; returns how many were removed. A folder
/// it cannot account for (a record this version cannot read) may belong to a VM still running:
/// it stays, as it does at start-up.
pub fn cleanup_finished_jobs(ctx: &AppCtx) -> usize {
    let jobs = ctx.config.jobs();
    ctx.store
        .list()
        .into_iter()
        .filter(|r| r.state.is_terminal() && jobs.join(r.id.as_str()).exists())
        .filter(|r| crate::adapters::jobdir::JobWorkspace::remove_job(&jobs, r.id.as_str()).is_ok())
        .count()
}
