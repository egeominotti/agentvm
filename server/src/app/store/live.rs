//! What a running VM reports, kept on its task as it changes: activity, metrics, usage, boot
//! log, tailnet, ports. Only real changes notify the dashboards.

use super::Store;
use crate::domain::ids::TaskId;
use crate::domain::metrics::{ForwardedPort, VmMetrics};
use crate::domain::telemetry::TelemetrySample;
use crate::domain::usage::AgentUsage;

impl Store {
    pub fn set_activity(&self, id: &TaskId, activity: Option<String>) {
        let changed = self.tasks.lock().unwrap().get_mut(id).is_some_and(|e| {
            let changed = e.record.activity != activity;
            e.record.activity = activity;
            changed
        });
        if changed {
            self.changes.bump();
        }
    }

    pub fn record_metrics(&self, id: &TaskId, m: VmMetrics) {
        if let Some(e) = self.tasks.lock().unwrap().get_mut(id) {
            e.record.push_metrics(m);
        }
    }

    /// A new telemetry sample for the live charts, with the memory the VM may keep now.
    pub fn record_sample(&self, id: &TaskId, sample: TelemetrySample, memory_limit_mb: u64) {
        if let Some(e) = self.tasks.lock().unwrap().get_mut(id) {
            e.record.push_live(sample, memory_limit_mb);
        }
    }

    /// Memory reserved by the VMs that hold a slot.
    pub fn committed_memory_mb(&self) -> u64 {
        self.tasks.lock().unwrap().values().filter(|e| e.record.holds_vm()).map(|e| e.record.memory_mb).sum()
    }

    /// Saved to disk only when it changes.
    pub fn set_auto_snapshot_min(&self, id: &TaskId, minutes: Option<u32>) {
        self.update(id, |r| ((), std::mem::replace(&mut r.auto_snapshot_min, minutes) != minutes));
    }

    pub fn set_usage(&self, id: &TaskId, usage: AgentUsage) {
        self.update(id, |r| {
            let changed = r.usage.as_ref() != Some(&usage);
            r.usage = Some(usage);
            ((), changed)
        });
    }

    pub fn push_boot(&self, id: &TaskId, line: String) {
        if let Some(e) = self.tasks.lock().unwrap().get_mut(id) {
            e.record.boot_log.push(line);
        }
    }

    /// Replaces the guest part of the boot log (the host lines stay first).
    pub fn set_guest_boot(&self, id: &TaskId, lines: Vec<String>, ready: bool) {
        if let Some(e) = self.tasks.lock().unwrap().get_mut(id) {
            e.record.boot_log.retain(|l| l.starts_with("host: "));
            e.record.boot_log.extend(lines);
            if std::mem::replace(&mut e.record.ready, ready) != ready {
                self.changes.bump();
            }
        }
    }

    /// The VM joins (or left) the tailnet: kept across restarts; its old report is forgotten.
    pub fn set_tailscale(&self, id: &TaskId, on: bool) {
        self.update(id, |r| {
            r.tailnet = None;
            ((), std::mem::replace(&mut r.tailscale, on) != on)
        });
    }

    pub fn set_tailnet(&self, id: &TaskId, tailnet: Option<crate::domain::tailscale::Tailnet>) {
        if let Some(e) = self.tasks.lock().unwrap().get_mut(id)
            && e.record.tailnet != tailnet
        {
            e.record.tailnet = tailnet;
            self.changes.bump();
        }
    }

    pub fn set_ports(&self, id: &TaskId, ports: Vec<ForwardedPort>) {
        if let Some(e) = self.tasks.lock().unwrap().get_mut(id)
            && e.record.ports != ports
        {
            e.record.ports = ports;
            self.changes.bump();
        }
    }
}
