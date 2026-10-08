//! Idle terminals give memory back to the Mac (balloon); any work gets it all back at once.

use std::time::Instant;

use crate::adapters::jobdir::JobWorkspace;
use crate::domain::memory::memory_target;
use crate::domain::metrics::VmMetrics;

/// A VM using more CPU than this (in %) counts as busy and keeps all its memory.
const BUSY_CPU_PCT: f64 = 10.0;

pub struct Balloon {
    /// Memory the VM was launched with.
    memory_mb: u64,
    /// Last moment the VM was busy.
    idle_since: Instant,
    /// Memory the guest was last told it may keep.
    target_mb: u64,
}

impl Balloon {
    /// `applied`: what the VM was last told it may keep (`memory.target`), if anything. A VM
    /// found after a restart may still be shrunk: starting from "all of it" would never give
    /// it back its memory once busy, as nothing would seem to change.
    pub fn new(memory_mb: u64, applied: Option<u64>) -> Self {
        Balloon { memory_mb, idle_since: Instant::now(), target_mb: applied.unwrap_or(memory_mb) }
    }

    /// Moves the balloon to the target for the latest sample, if that target changed.
    /// The memory the VM is allowed to keep right now.
    pub fn target_mb(&self) -> u64 {
        self.target_mb
    }

    pub fn adjust(&mut self, ws: &JobWorkspace, m: &VmMetrics, working: bool) {
        let busy = working || m.cpu_pct > BUSY_CPU_PCT;
        if busy {
            self.idle_since = Instant::now();
        }
        let target = memory_target(self.memory_mb, m.mem_used_mb, busy, self.idle_since.elapsed().as_secs());
        if target != self.target_mb && ws.set_memory_target(target).is_ok() {
            self.target_mb = target;
        }
    }
}
