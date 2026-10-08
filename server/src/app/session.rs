//! Actions on a running terminal: open a PTY, save the work to the branch, close the VM.

use std::time::Duration;

use super::supervisor::AppCtx;
use crate::adapters::git::Git;
use crate::adapters::jobdir::JobWorkspace;
use crate::adapters::pty::PtyConnection;

/// Terminal types exposed to HTTP through `app`.
pub use crate::adapters::pty::{Frame as TerminalInput, PtyConnection as Terminal};
use crate::domain::ids::TaskId;
use crate::domain::task::TaskState;

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
}

fn running_terminal(ctx: &AppCtx, id: &TaskId) -> Result<crate::app::store::TaskRecord, SessionError> {
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
) -> Result<PtyConnection, SessionError> {
    running_terminal(ctx, id)?;
    let socket = JobWorkspace::pty_socket_of(&ctx.config.jobs(), id);
    PtyConnection::open(&socket, session, cols, rows).await.map_err(|e| SessionError::Unreachable(e.to_string()))
}

/// Asks the guest to commit and bundle, then updates `agent/<id>` in the repo. Returns the commit count.
pub async fn save(ctx: &AppCtx, id: &TaskId) -> Result<u32, SessionError> {
    let record = running_terminal(ctx, id)?;
    let share = JobWorkspace::share_of(&ctx.config.jobs(), id);
    let done = share.join("save.done");
    let _ = std::fs::remove_file(&done);
    std::fs::write(share.join("save.request"), "").map_err(|e| SessionError::SaveFailed(e.to_string()))?;

    let t0 = tokio::time::Instant::now();
    let commits = loop {
        if let Ok(text) = std::fs::read_to_string(&done) {
            let _ = std::fs::remove_file(&done);
            let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            break v["commits"].as_u64().unwrap_or(0) as u32;
        }
        if t0.elapsed() > SAVE_TIMEOUT {
            return Err(SessionError::SaveFailed("the VM did not respond in time".into()));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    if commits > 0 {
        let bundle = share.join("out.bundle");
        let branch = id.branch();
        tokio::task::spawn_blocking(move || Git::new(record.repo).fetch_bundle(&bundle, &branch))
            .await
            .map_err(|e| SessionError::SaveFailed(e.to_string()))?
            .map_err(|e| SessionError::SaveFailed(e.to_string()))?;
    }
    Ok(commits)
}

/// Final save and shutdown: the supervisor collects the outcome as for any task.
pub fn close(ctx: &AppCtx, id: &TaskId) -> Result<(), SessionError> {
    running_terminal(ctx, id)?;
    let share = JobWorkspace::share_of(&ctx.config.jobs(), id);
    std::fs::write(share.join("close.request"), "").map_err(|e| SessionError::Unreachable(e.to_string()))
}
