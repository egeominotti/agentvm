//! Snapshots of running terminals and restoring them into new VMs.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::supervisor::{AppCtx, NewTask, SubmitError, random_bytes, submit};
use crate::adapters::jobdir::JobWorkspace;
use crate::domain::ids::TaskId;
use crate::domain::settings::Model;
use crate::domain::snapshot::{AutoSnapshots, SnapshotId, SnapshotMeta};
use crate::domain::task::TaskState;
use super::store::TaskRecord;

const SYNC_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("task not found")]
    NotFound,
    #[error("only a running terminal can be snapshotted")]
    NotRunning,
    #[error("snapshot not found")]
    NoSnapshot,
    #[error("automatic snapshots run every 1, 5, 10, 15, 30, 60, 120 or 240 minutes, or never (0)")]
    Interval,
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
    take(ctx, id, name, None).await
}

/// An automatic snapshot (`why`: "auto", "before close"), then pruning to the newest `keep`.
pub async fn take_auto(ctx: &AppCtx, id: &TaskId, why: &str) -> Result<SnapshotMeta, SnapshotError> {
    let meta = take(ctx, id, None, Some(why)).await?;
    let policy = ctx.settings.get().auto_snapshots;
    for old in policy.to_prune(&ctx.snapshots.list(), id.as_str()) {
        let _ = ctx.snapshots.delete(&old);
    }
    Ok(meta)
}

/// Every 15 seconds: snapshots the running terminals whose interval has elapsed.
pub async fn run_schedule(ctx: Arc<AppCtx>) {
    loop {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let policy = ctx.settings.get().auto_snapshots;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        let snapshots = ctx.snapshots.list();
        for record in ctx.store.list() {
            if !(record.interactive && record.ready && record.state == TaskState::Running) {
                continue;
            }
            let last = snapshots
                .iter()
                .filter(|s| s.auto && s.source_task == record.id.as_str())
                .map(|s| s.created_at)
                .fold(None, |m: Option<f64>, t| Some(m.map_or(t, |m| m.max(t))));
            let started = record.created_at.duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(now);
            let policy = AutoSnapshots { every_min: record.auto_snapshot_min.unwrap_or(policy.every_min), ..policy };
            if policy.due(last, started, now) {
                let _ = take_auto(&ctx, &record.id, "auto").await;
            }
        }
    }
}

/// `auto`: why an automatic snapshot is taken (it goes into its name), `None` for a manual one.
async fn take(
    ctx: &AppCtx,
    id: &TaskId,
    name: Option<String>,
    auto: Option<&str>,
) -> Result<SnapshotMeta, SnapshotError> {
    let record = ctx.store.get(id).ok_or(SnapshotError::NotFound)?;
    if !record.interactive || record.state != TaskState::Running {
        return Err(SnapshotError::NotRunning);
    }
    let jobs = ctx.config.jobs();
    let share = JobWorkspace::share_of(&jobs, id);
    let done = share.join("sync.done");
    let _ = std::fs::remove_file(&done);
    crate::adapters::jobdir::write_request(&share, "sync.request")?;
    let t0 = tokio::time::Instant::now();
    while !done.exists() {
        if t0.elapsed() > SYNC_TIMEOUT {
            return Err(SnapshotError::SyncTimeout);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let _ = std::fs::remove_file(&done);

    let now = SystemTime::now();
    let title = title(&record, now);
    let name = match auto {
        Some(why) => format!("{title}, {why} {}", clock(now)),
        None => name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()).unwrap_or(title),
    };
    let meta = meta_for(&record, name, auto.is_some(), now);
    let (disk, efivars) = (JobWorkspace::disk_of(&jobs, id), JobWorkspace::efivars_of(&jobs, id));
    let store_meta = meta.clone();
    let root = ctx.config.home.join("snapshots");
    tokio::task::spawn_blocking(move || {
        crate::adapters::snapshots::SnapshotStore::new(root).create(&store_meta, &disk, &efivars)
    })
    .await
    .map_err(|e| SnapshotError::Io(std::io::Error::other(e.to_string())))?
    .map_err(SnapshotError::Io)
}

/// What the VM was for: the first line of its task, or its repository and start time.
fn title(record: &TaskRecord, now: SystemTime) -> String {
    let repo_name =
        record.repo.as_path().file_name().map_or_else(|| "repository".into(), |n| n.to_string_lossy().into_owned());
    match (&record.prompt, &record.label) {
        (Some(p), _) => p.as_str().lines().next().unwrap_or_default().to_owned(),
        (None, Some(label)) => label.trim_start_matches("Restored: ").to_owned(),
        (None, None) => format!("{repo_name}, {}", clock(now)),
    }
}

fn meta_for(record: &TaskRecord, name: String, auto: bool, now: SystemTime) -> SnapshotMeta {
    SnapshotMeta {
        id: SnapshotId::generate(now, random_bytes()),
        name,
        source_task: record.id.to_string(),
        repo: record.repo.as_path().display().to_string(),
        base_sha: record.base_sha.as_str().to_owned(),
        model: record.model.as_str().to_owned(),
        claude_version: record.claude_version.clone(),
        created_at: now.duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0),
        size_mb: 0,
        cpus: record.cpus,
        memory_mb: record.memory_mb,
        auto,
    }
}

/// The disk of a VM that ended without its work reaching the repository (a reboot, a crash, a
/// failed import): kept as a snapshot it can be restored from, never deleted. Returns its name.
pub fn keep_disk(ctx: &AppCtx, record: &TaskRecord, ws: &JobWorkspace) -> Option<String> {
    let (disk, efivars) = (ws.disk(), ws.efivars());
    if !disk.is_file() || !efivars.is_file() {
        return None;
    }
    let now = SystemTime::now();
    let meta = meta_for(record, format!("Interrupted: {}", title(record, now)), false, now);
    ctx.snapshots.create(&meta, &disk, &efivars).ok().map(|m| m.name)
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
            cpus: (meta.cpus > 0).then_some(meta.cpus),
            memory_mb: (meta.memory_mb > 0).then_some(meta.memory_mb),
            label: Some(format!("Restored: {}", meta.name)),
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

/// Local wall-clock time `HH:MM` (UTC offset from the system's `date`), for default names.
fn clock(now: SystemTime) -> String {
    let secs = now.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let offset = std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()
        .and_then(|o| {
            let z = String::from_utf8_lossy(&o.stdout).trim().to_owned();
            let sign = if z.starts_with('-') { -1 } else { 1 };
            let digits: i64 = z.trim_start_matches(['+', '-']).parse().ok()?;
            Some(sign * ((digits / 100) * 3600 + (digits % 100) * 60))
        })
        .unwrap_or(0);
    let local = (secs as i64 + offset).rem_euclid(86_400);
    format!("{:02}:{:02}", local / 3600, local / 60 % 60)
}

/// Minutes between automatic snapshots of one machine; `None` follows the settings.
pub fn set_interval(ctx: &AppCtx, id: &TaskId, minutes: Option<u32>) -> Result<(), SnapshotError> {
    ctx.store.get(id).ok_or(SnapshotError::NotFound)?;
    if let Some(m) = minutes
        && !AutoSnapshots::INTERVALS.contains(&m)
    {
        return Err(SnapshotError::Interval);
    }
    ctx.store.set_auto_snapshot_min(id, minutes);
    Ok(())
}
