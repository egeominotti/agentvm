//! Supervising a running VM: one supervisor (tokio task) per task, from boot to the result.

mod backstop;
mod balloon;
mod follower;
mod tick;
mod wait;

use self::backstop::Backstop;
use self::follower::Follower;
use self::tick::Ticker;
use self::wait::wait_for_vm;
use super::collect::collect;
use super::context::AppCtx;
use super::record::TaskRecord;
use crate::adapters::jobdir::JobWorkspace;
use crate::adapters::vm::VmProcess;
use crate::domain::ids::TaskId;
use crate::domain::task::TaskEvent;
use crate::secret::Secret;

/// Follows a running VM until it stops, then collects the result. Shared by new VMs and by VMs
/// re-attached after a server restart.
pub(super) async fn supervise(
    ctx: &AppCtx,
    id: &TaskId,
    record: &TaskRecord,
    ws: JobWorkspace,
    mut vm: VmProcess,
    token: Secret,
) -> Result<(), String> {
    let on_cost = cost_recorder(ctx, id);
    let follower = Follower::start(ctx.store.log(id).ok_or("task disappeared")?, ws.stream(), token, on_cost);
    let on_started = || {
        let _ = ctx.store.apply(id, TaskEvent::VmStarted);
    };
    let mut ticker = Ticker::new(ctx, id, record, &ws);
    let mut backstop = Backstop::new(ctx, record, &ws);
    let deadline_of = || backstop.as_mut().map(Backstop::deadline);
    let end = wait_for_vm(ctx, id, &mut vm, deadline_of, on_started, || ticker.tick()).await;
    ctx.store.set_activity(id, None);
    ctx.forwards.stop_all(id);
    ctx.store.set_ports(id, Vec::new());
    follower.finish().await;
    collect(ctx, id, record, &ws, end)
}

/// Keeps the task's cost in step with the agent's `result` events.
fn cost_recorder(ctx: &AppCtx, id: &TaskId) -> impl Fn(f64) + Send + 'static {
    let (store, id) = (ctx.store.clone_handle(), id.clone());
    move |cost_usd: f64| {
        let mut usage = store.get(&id).and_then(|r| r.usage).unwrap_or_default();
        usage.cost_usd = cost_usd;
        store.set_usage(&id, usage);
    }
}
