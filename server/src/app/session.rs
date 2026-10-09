//! Actions on a running terminal: open a PTY, save the work to the branch, close the VM.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::context::AppCtx;
use super::guest_channel::{Answer, AskError};
use crate::adapters::git::Git;
use crate::adapters::jobdir::JobWorkspace;
use crate::adapters::pty::PtyConnection;

/// Terminal types exposed to HTTP through `app`.
pub use crate::adapters::pty::{Frame as TerminalInput, PtyConnection as Terminal};
use crate::domain::ids::TaskId;
use crate::domain::save::SaveReply;
use crate::domain::task::TaskState;
use crate::guestfs;

/// A save commits and bundles the whole checkout: minutes on a big repository on a busy Mac.
const SAVE_TIMEOUT: Duration = Duration::from_secs(300);

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
    #[error(
        "the VM stays open, nothing is lost: the snapshot before closing failed ({0}). To close without it, turn off \"Snapshot before closing\" in Settings"
    )]
    SnapshotFailed(String),
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
    let not_running = || ctx.store.get(id).is_none_or(|r| r.state != TaskState::Running);
    let answer = ctx.guest.ask(&jobs, id, "save", &["save.done"], SAVE_TIMEOUT, not_running).await;
    let reply = match answer {
        Ok(Answer::Reply { bytes, .. }) => {
            SaveReply::parse(&bytes).ok_or_else(|| SessionError::SaveFailed("unreadable reply".into()))?
        }
        Ok(Answer::Gone) => return Err(SessionError::NotRunning),
        Err(AskError::Timeout) => return Err(SessionError::SaveFailed("the VM did not respond in time".into())),
        Err(e) => return Err(SessionError::SaveFailed(e.to_string())),
    };
    let commits = match reply {
        SaveReply::Saved { commits } => commits,
        SaveReply::Failed(e) => return Err(SessionError::SaveFailed(e)),
    };
    let mut branch = id.branch();
    if commits > 0 {
        // Imported from a copy only the Mac controls, not from the file the guest can swap.
        // A copy of its own per save: two saves at once never truncate each other's file.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let from = JobWorkspace::out_bundle_of(&jobs, id);
        let copy = jobs.join(id.as_str()).join(format!("saved-{n}.bundle"));
        let (target, room) = (branch.clone(), ctx.room_bytes());
        branch = tokio::task::spawn_blocking(move || {
            let imported = guestfs::copy_out(&from, &copy, room)
                .map_err(|e| e.to_string())
                .and_then(|_| Git::new(record.repo).import_bundle(&copy, &target).map_err(|e| e.to_string()));
            let _ = std::fs::remove_file(&copy);
            imported
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
const CLOSE_TIMEOUT: Duration = Duration::from_secs(300);

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
    // Promised by the setting: without it the VM is not closed (its disk would be deleted).
    if ctx.settings.get().auto_snapshots.before_close
        && let Err(e) = super::snapshots::take_auto(ctx, id, "before close").await
    {
        tracing::warn!(task = %id, error = %e, "snapshot before closing failed: the VM stays open");
        return Err(SessionError::SnapshotFailed(e.to_string()));
    }
    let not_running = || ctx.store.get(id).is_none_or(|r| r.state != TaskState::Running);
    // Success has no reply: the guest saves, then powers off.
    let answer = ctx.guest.ask(&ctx.config.jobs(), id, "close", &["close.failed"], CLOSE_TIMEOUT, not_running).await;
    match answer {
        Ok(Answer::Gone) => Ok(()),
        Ok(Answer::Reply { bytes, .. }) => {
            let reply: Value = serde_json::from_slice(&bytes).unwrap_or_default();
            Err(SessionError::CloseFailed(reply["error"].as_str().unwrap_or("unknown error").to_owned()))
        }
        Err(AskError::Timeout) => {
            Err(SessionError::Unreachable("the VM has not finished its final save yet; it closes when it does".into()))
        }
        Err(e) => Err(SessionError::Unreachable(e.to_string())),
    }
}

/// Closes an extra shell of a running terminal: its tmux session in the VM ends.
pub async fn close_shell(ctx: &AppCtx, id: &TaskId, session: &str) -> Result<(), SessionError> {
    running_terminal(ctx, id)?;
    let socket = JobWorkspace::pty_socket_of(&ctx.config.jobs(), id);
    PtyConnection::kill(&socket, session).await.map_err(|e| SessionError::Unreachable(e.to_string()))
}

/// A file dropped on a terminal, being written into the VM's shared folder.
pub struct Upload {
    /// The file being written; `discard` it if the upload fails.
    pub file: guestfs::NewFile,
    /// Where the VM sees it: `/mnt/job/uploads/<name>`.
    pub guest_path: String,
}

/// Starts an upload into the running terminal `id`; a taken name gets ` (2)`, ` (3)`…
pub fn start_upload(ctx: &AppCtx, id: &TaskId, name: &str) -> Result<Upload, SessionError> {
    running_terminal(ctx, id)?;
    let dir = JobWorkspace::share_of(&ctx.config.jobs(), id).join("uploads");
    let base = std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let file = guestfs::create_unique(&dir, &base).map_err(|e| SessionError::Unreachable(e.to_string()))?;
    Ok(Upload { guest_path: format!("/mnt/job/uploads/{}", file.name), file })
}
