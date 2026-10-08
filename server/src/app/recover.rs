//! After a restart: reload every task, re-attach to VMs that kept running, finish the others.

use std::collections::HashSet;
use std::sync::Arc;

use super::collect::{VmEnd, collect};
use super::context::AppCtx;
use super::launch;
use super::scheduler::Slot;
use super::store::Store;
use super::supervise::supervise;
use crate::adapters::jobdir::JobWorkspace;
use crate::adapters::vm::VmProcess;
use crate::domain::ids::TaskId;
use crate::domain::outcome::VmExit;
use crate::domain::task::{TaskEvent, TaskState};
use crate::secret::Secret;

/// Returns the job ids whose VM is (still) owned by a task, so orphan cleanup leaves them alone.
pub fn recover(ctx: &Arc<AppCtx>) -> HashSet<String> {
    let (records, unreadable) = Store::load_all(&ctx.config.jobs());
    // A record this version cannot read may still own a running VM: never treat it as an orphan.
    let mut live: HashSet<String> = unreadable.into_iter().collect();
    let mut queued = Vec::new();
    for record in records {
        let (id, state, memory_mb) = (record.id.clone(), record.state.clone(), record.memory_mb);
        ctx.store.insert(record);
        match state {
            s if s.is_terminal() => {}
            TaskState::Queued => queued.push(id),
            TaskState::Preparing => {
                let _ = ctx.store.apply(&id, TaskEvent::Failure("interrupted while preparing: launch it again".into()));
            }
            _ => {
                live.insert(id.to_string());
                // The VM is already running: it takes a slot now (if any is left), before the queue.
                let slot = ctx.scheduler.try_acquire(memory_mb);
                tokio::spawn(resume(ctx.clone(), id, slot));
            }
        }
    }
    for id in queued {
        tokio::spawn(launch::run(ctx.clone(), id));
    }
    live
}

/// Supervises a VM that survived the restart, or collects what one that did not left behind.
async fn resume(ctx: Arc<AppCtx>, id: TaskId, _slot: Option<Slot>) {
    let Some(record) = ctx.store.get(&id) else { return };
    let ws = JobWorkspace::existing(&ctx.config.jobs(), &id);
    let vm = ws.read_pid().and_then(|pid| VmProcess::attach(pid, &ws.events()));
    let result = match vm {
        Some(vm) => {
            let keychain = ctx.keychain.clone();
            let token = tokio::task::spawn_blocking(move || keychain.read_token())
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_else(|| Secret::new(String::new()));
            supervise(&ctx, &id, &record, ws, vm, token).await
        }
        None => {
            // The VM stopped while the server was down: whatever it left behind decides the outcome.
            let exit = if ws.read_result().is_some() {
                VmExit::Clean
            } else {
                VmExit::Error("the VM stopped while agentvm was not running".into())
            };
            collect(&ctx, &id, &record, &ws, VmEnd::unprompted(exit))
        }
    };
    if let Err(reason) = result {
        let _ = ctx.store.apply(&id, TaskEvent::Failure(reason));
    }
}
