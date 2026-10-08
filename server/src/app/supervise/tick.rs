//! The once-a-second refresh of a running VM: boot log, activity, memory, ports, metrics, usage.

use crate::adapters::jobdir::JobWorkspace;
use crate::app::balloon::Balloon;
use crate::app::context::AppCtx;
use crate::app::proxy::vm_name;
use crate::app::record::TaskRecord;
use crate::domain::ids::TaskId;
use crate::domain::metrics::VmMetrics;

/// Reads what the guest wrote to the job folder and publishes it on the task.
pub(super) struct Ticker<'a> {
    ctx: &'a AppCtx,
    id: &'a TaskId,
    record: &'a TaskRecord,
    ws: &'a JobWorkspace,
    balloon: Balloon,
}

impl<'a> Ticker<'a> {
    pub(super) fn new(ctx: &'a AppCtx, id: &'a TaskId, record: &'a TaskRecord, ws: &'a JobWorkspace) -> Self {
        Ticker { ctx, id, record, ws, balloon: Balloon::new(record.memory_mb, ws.memory_target()) }
    }

    pub(super) fn tick(&mut self) {
        self.refresh_boot_log();
        let working = self.refresh_activity();
        if let Some(m) = self.ws.read_metrics() {
            if self.record.interactive {
                self.balloon.adjust(self.ws, &m, working);
                self.sync_ports(&m);
            }
            self.ctx.store.record_metrics(self.id, m);
        }
        if let Some(u) = self.ws.read_usage() {
            self.ctx.store.set_usage(self.id, u);
        }
    }

    /// Until the VM is ready, mirrors the guest job's log into the boot timeline.
    fn refresh_boot_log(&self) {
        if self.ctx.store.get(self.id).is_some_and(|r| r.ready) {
            return;
        }
        let lines = self.ws.job_log();
        let marker = if self.record.interactive { "terminal ready" } else { "network ready" };
        let ready = lines.iter().any(|l| l.ends_with(marker));
        self.ctx.store.set_guest_boot(self.id, lines, ready);
    }

    /// Publishes what the Claude Code hooks reported last; `true` while the agent is working.
    fn refresh_activity(&self) -> bool {
        let activity = self.ws.activity();
        let working = activity.as_deref() == Some("working");
        self.ctx.store.set_activity(self.id, activity);
        working
    }

    /// Makes the ports the guest listens on reachable from the Mac.
    fn sync_ports(&self, m: &VmMetrics) {
        let socket = JobWorkspace::pty_socket_of(&self.ctx.config.jobs(), self.id);
        let vm = vm_name(self.record);
        let ports = self.ctx.forwards.sync(socket, self.id, &vm, self.ctx.config.port, &m.ports);
        self.ctx.store.set_ports(self.id, ports);
    }
}
