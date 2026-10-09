//! The VM is gone: decide the outcome, import the branch if needed, keep the disk of a failure,
//! finish the task.

use super::context::AppCtx;
use super::record::TaskRecord;
use super::snapshots;
use crate::adapters::git::Git;
use crate::adapters::jobdir::JobWorkspace;
use crate::domain::ids::TaskId;
use crate::domain::outcome::{Final, OutcomeInput, VmExit, decide};
use crate::domain::task::{TaskEvent, TaskState};
use crate::guestfs;

/// How the VM ended, and whether the host asked it to.
pub(super) struct VmEnd {
    pub exit: VmExit,
    pub stop_requested: bool,
    pub timed_out: bool,
}

impl VmEnd {
    /// The VM stopped on its own (or while nobody was watching).
    pub fn unprompted(exit: VmExit) -> Self {
        VmEnd { exit, stop_requested: false, timed_out: false }
    }
}

/// Finishes the task from what the VM left behind in its job folder.
pub(super) fn collect(
    ctx: &AppCtx,
    id: &TaskId,
    record: &TaskRecord,
    ws: &JobWorkspace,
    end: VmEnd,
) -> Result<(), String> {
    let apply = |e| ctx.store.apply(id, e).map(drop).map_err(|e| e.to_string());
    if ctx.store.get(id).is_some_and(|r| r.state != TaskState::Collecting) {
        apply(TaskEvent::VmExited)?;
    }
    let exit = match end.exit {
        VmExit::Error(m) => VmExit::Error(with_console(m, &ws.console_tail(5))),
        other => other,
    };
    let outcome = decide(&OutcomeInput {
        exit,
        result: ws.read_result(),
        stop_requested: end.stop_requested,
        timed_out: end.timed_out,
        has_out_bundle: ws.has_out_bundle(),
    });
    let (final_, branch) = match (outcome.fetch, outcome.final_) {
        (true, f) => match import_branch(ws, record, &id.branch(), ctx.room_bytes()) {
            Ok(landed) => {
                tracing::info!(task = %id, branch = %landed, "work imported into the repository");
                (f, landed)
            }
            Err(e) => {
                tracing::error!(task = %id, error = %e, "the work could not be imported");
                (Final::Failed(format!("fetch_failed: {e}")), id.branch())
            }
        },
        (false, f) => (f, id.branch()),
    };
    let final_ = match final_ {
        Final::Failed(reason) if !end.stop_requested => Final::Failed(keep_disk(ctx, record, ws, reason)),
        f => f,
    };
    apply(TaskEvent::Finished(final_, branch))
}

/// Whatever went wrong, the disk may hold work that never reached the repository: keep it
/// (an instant clone) instead of deleting it with the job. Only a Stop asked for discards it.
/// Returns the failure reason, saying where the disk went.
fn keep_disk(ctx: &AppCtx, record: &TaskRecord, ws: &JobWorkspace, reason: String) -> String {
    match tokio::task::block_in_place(|| snapshots::keep_disk(ctx, record, ws)) {
        Some(name) => {
            tracing::warn!(task = %record.id, snapshot = %name, "the disk of a failed VM is kept as a snapshot");
            format!("{reason}. Its disk is kept in Snapshots as \"{name}\"")
        }
        None => reason,
    }
}

/// Imports the guest's work into the repository; returns the branch it landed on.
/// Imported from a copy only the Mac controls, not from the file the guest could swap.
fn import_branch(ws: &JobWorkspace, record: &TaskRecord, branch: &str, room: u64) -> Result<String, String> {
    // git runs for seconds on big repos: tell the runtime this worker is blocked.
    tokio::task::block_in_place(|| {
        let copy = ws.dir().join("final.bundle");
        guestfs::copy_out(&ws.out_bundle(), &copy, room).map_err(|e| e.to_string())?;
        Git::new(record.repo.clone()).import_bundle(&copy, branch).map_err(|e| e.to_string())
    })
}

fn with_console(message: String, console_tail: &str) -> String {
    if console_tail.trim().is_empty() { message } else { format!("{message}\n--- console ---\n{console_tail}") }
}
