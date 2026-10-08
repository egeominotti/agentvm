//! Snapshots of running terminals and restoring them into new VMs.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::supervisor::{AppCtx, NewTask, SubmitError, random_bytes, submit};
use crate::adapters::jobdir::JobWorkspace;
use crate::domain::ids::TaskId;
use crate::domain::settings::Model;
use crate::domain::snapshot::{SnapshotId, SnapshotMeta};
use crate::domain::task::TaskState;

const SYNC_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("task not found")]
    NotFound,
    #[error("only a running terminal can be snapshotted")]
    NotRunning,
    #[error("snapshot not found")]
    NoSnapshot,
    #[error("the VM did not flush its disk in time")]
    SyncTimeout,
    #[error("could not save the snapshot: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Submit(#[from] SubmitError),
}

/// Flushes the guest's disk cache, then clones the disk: equivalent to pulling the plug at that
/// instant, which ext4's journal handles. The VM keeps running.
pub async fn take_snapshot(ctx: &AppCtx, id: &TaskId, name: Option<String>) -> Result<SnapshotMeta, SnapshotError> {
    let record = ctx.store.get(id).ok_or(SnapshotError::NotFound)?;
    if !record.interactive || record.state != TaskState::Running {
        return Err(SnapshotError::NotRunning);
    }
    let jobs = ctx.config.jobs();
    let share = JobWorkspace::share_of(&jobs, id);
    let done = share.join("sync.done");
    let _ = std::fs::remove_file(&done);
    std::fs::write(share.join("sync.request"), "")?;
    let t0 = tokio::time::Instant::now();
    while !done.exists() {
        if t0.elapsed() > SYNC_TIMEOUT {
            return Err(SnapshotError::SyncTimeout);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let _ = std::fs::remove_file(&done);

    let now = SystemTime::now();
    let title = record.prompt.as_ref().map_or_else(|| record.repo.as_path().display().to_string(), |p| p.as_str().lines().next().unwrap_or_default().to_owned());
    let meta = SnapshotMeta {
        id: SnapshotId::generate(now, random_bytes()),
        name: name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()).unwrap_or(title),
        source_task: id.to_string(),
        repo: record.repo.as_path().display().to_string(),
        base_sha: record.base_sha.as_str().to_owned(),
        model: record.model.as_str().to_owned(),
        claude_version: record.claude_version.clone(),
        created_at: now.duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0),
        size_mb: 0,
    };
    let (disk, efivars) = (JobWorkspace::disk_of(&jobs, id), JobWorkspace::efivars_of(&jobs, id));
    let store_meta = meta.clone();
    let root = ctx.config.home.join("snapshots");
    tokio::task::spawn_blocking(move || crate::adapters::snapshots::SnapshotStore::new(root).create(&store_meta, &disk, &efivars))
        .await
        .map_err(|e| SnapshotError::Io(std::io::Error::other(e.to_string())))?
        .map_err(SnapshotError::Io)
}

/// Starts a new terminal VM from the snapshot; Claude continues its last conversation.
pub fn restore(ctx: &Arc<AppCtx>, snap: &SnapshotId) -> Result<TaskId, SnapshotError> {
    let meta = ctx.snapshots.get(snap).ok_or(SnapshotError::NoSnapshot)?;
    let id = submit(
        ctx,
        NewTask {
            repo: &meta.repo,
            prompt: String::new(),
            base_ref: Some(&meta.base_sha),
            interactive: true,
            model: Model::parse(&meta.model).ok(),
            claude_version: None,
            restore_from: Some(snap.clone()),
        },
    )?;
    Ok(id)
}

pub fn list(ctx: &AppCtx) -> Vec<SnapshotMeta> {
    ctx.snapshots.list()
}

pub fn delete(ctx: &AppCtx, snap: &SnapshotId) -> Result<(), SnapshotError> {
    if ctx.snapshots.get(snap).is_none() {
        return Err(SnapshotError::NoSnapshot);
    }
    Ok(ctx.snapshots.delete(snap)?)
}
