//! Actions on a running terminal: open a PTY, save the work to the branch, close the VM.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::context::AppCtx;
use crate::adapters::git::Git;
use crate::adapters::jobdir::{JobWorkspace, write_request};
use crate::adapters::pty::PtyConnection;

/// Terminal types exposed to HTTP through `app`.
pub use crate::adapters::pty::{Frame as TerminalInput, PtyConnection as Terminal};
use crate::domain::ids::TaskId;
use crate::domain::save::SaveReply;
use crate::domain::task::TaskState;
use crate::guestfs;

const SAVE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("task not found")]
    NotFound,
    #[error("the terminal is not running")]
    NotRunning,
    #[error("this task is not a terminal")]
    NotInteractive,
    #[error("the VM is not responding: {0}")]
    Unreachable(String),
    #[error("save failed: {0}")]
    SaveFailed(String),
    #[error("the VM stays open, nothing is lost: its final save failed ({0})")]
    CloseFailed(String),
}

fn running_terminal(ctx: &AppCtx, id: &TaskId) -> Result<crate::app::record::TaskRecord, SessionError> {
    let record = ctx.store.get(id).ok_or(SessionError::NotFound)?;
    if !record.interactive {
        return Err(SessionError::NotInteractive);
    }
    if record.state != TaskState::Running {
        return Err(SessionError::NotRunning);
    }
    Ok(record)
}

/// Attaches a client to the VM's `claude` or `shell` session (created if it does not exist).
pub async fn open_terminal(
    ctx: &AppCtx,
    id: &TaskId,
    session: &str,
    cols: u16,
    rows: u16,
    view: bool,
) -> Result<PtyConnection, SessionError> {
    running_terminal(ctx, id)?;
    let socket = JobWorkspace::pty_socket_of(&ctx.config.jobs(), id);
    PtyConnection::open_with(&socket, session, cols, rows, view)
        .await
        .map_err(|e| SessionError::Unreachable(e.to_string()))
}

/// Asks the guest to commit and bundle, then imports the work into the repo.
pub async fn save(ctx: &AppCtx, id: &TaskId) -> Result<Saved, SessionError> {
    let record = running_terminal(ctx, id)?;
    let jobs = ctx.config.jobs();
    let share = JobWorkspace::share_of(&jobs, id);
    let done = share.join("save.done");
    let _ = std::fs::remove_file(&done);
    write_request(&share, "save.request").map_err(|e| SessionError::SaveFailed(e.to_string()))?;

    let t0 = tokio::time::Instant::now();
    let commits = loop {
        // Partial JSON means the guest is still writing: look again on the next round.
        if let Some(reply) = guestfs::read(&done, 4096).and_then(|b| SaveReply::parse(&b)) {
            let _ = std::fs::remove_file(&done);
            match reply {
                SaveReply::Saved { commits } => break commits,
                SaveReply::Failed(e) => return Err(SessionError::SaveFailed(e)),
            }
        }
        if t0.elapsed() > SAVE_TIMEOUT {
            return Err(SessionError::SaveFailed("the VM did not respond in time".into()));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    let mut branch = id.branch();
    if commits > 0 {
        // Imported from a copy only the Mac controls, not from the file the guest can swap.
        let (from, copy) =
            (JobWorkspace::existing(&jobs, id).out_bundle(), jobs.join(id.as_str()).join("saved.bundle"));
        let target = branch.clone();
        branch = tokio::task::spawn_blocking(move || {
            guestfs::copy_out(&from, &copy).map_err(|e| e.to_string())?;
            Git::new(record.repo).import_bundle(&copy, &target).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| SessionError::SaveFailed(e.to_string()))?
        .map_err(SessionError::SaveFailed)?;
    }
    Ok(Saved { commits, branch })
}

/// What a save produced: `branch` is where the work landed (see `Git::import_bundle`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub commits: u32,
    pub branch: String,
}

/// Longest wait for the guest's final save before telling the user the VM is still closing.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(120);

/// Final save and shutdown. Returns once the VM is going away (the supervisor then collects the
/// outcome as for any task), or why it stays up: its final save failed and nothing is lost. Runs
/// on a task of its own, so a page reloaded meanwhile cannot leave the close half done.
pub async fn close(ctx: &Arc<AppCtx>, id: &TaskId) -> Result<(), SessionError> {
    running_terminal(ctx, id)?;
    let (ctx, id) = (ctx.clone(), id.clone());
    tokio::spawn(async move { close_and_wait(&ctx, &id).await })
        .await
        .map_err(|e| SessionError::Unreachable(e.to_string()))?
}

async fn close_and_wait(ctx: &AppCtx, id: &TaskId) -> Result<(), SessionError> {
    // The whole machine as it was (installed packages, Claude's conversation), not just the branch.
    if ctx.settings.get().auto_snapshots.before_close {
        let _ = super::snapshots::take_auto(ctx, id, "before close").await;
    }
    let share = JobWorkspace::share_of(&ctx.config.jobs(), id);
    let failed = share.join("close.failed");
    let _ = std::fs::remove_file(&failed);
    write_request(&share, "close.request").map_err(|e| SessionError::Unreachable(e.to_string()))?;
    let t0 = tokio::time::Instant::now();
    loop {
        // Partial JSON means the guest is still writing: look again on the next round.
        if let Some(reply) = guestfs::read(&failed, 4096).and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
            let _ = std::fs::remove_file(&failed);
            let reason = reply["error"].as_str().unwrap_or("unknown error").to_owned();
            return Err(SessionError::CloseFailed(reason));
        }
        if ctx.store.get(id).is_none_or(|r| r.state != TaskState::Running) {
            return Ok(());
        }
        if t0.elapsed() > CLOSE_TIMEOUT {
            let late = "the VM has not finished its final save yet; it closes when it does";
            return Err(SessionError::Unreachable(late.into()));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// A file dropped on a terminal, being written into the VM's shared folder.
pub struct Upload {
    pub file: std::fs::File,
    /// Where the VM sees it: `/mnt/job/uploads/<name>`.
    pub guest_path: String,
    pub host_path: std::path::PathBuf,
}

/// Starts an upload into the running terminal `id`; a taken name gets ` (2)`, ` (3)`…
pub fn start_upload(ctx: &AppCtx, id: &TaskId, name: &str) -> Result<Upload, SessionError> {
    running_terminal(ctx, id)?;
    let dir = JobWorkspace::share_of(&ctx.config.jobs(), id).join("uploads");
    let base = std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (file, name) = guestfs::create_unique(&dir, &base).map_err(|e| SessionError::Unreachable(e.to_string()))?;
    Ok(Upload { file, guest_path: format!("/mnt/job/uploads/{name}"), host_path: dir.join(name) })
}
