//! The server's time limit for an automatic task. The guest enforces the limit itself and saves
//! the work; this is the backstop if it cannot. It counts from when Claude starts, as the guest
//! does: boot and `setup.sh` can take long. After a server restart it still counts from Claude's
//! real start (the guest's uptime then and now), never from the restart.

use std::time::{Duration, Instant};

use crate::adapters::jobdir::JobWorkspace;
use crate::app::context::AppCtx;
use crate::app::record::TaskRecord;
use crate::domain::outcome::{PREPARE_MAX, TIMEOUT_GRACE, claude_start_uptime};

pub(super) struct Backstop<'a> {
    ws: &'a JobWorkspace,
    limit: Duration,
    /// When this server started following the VM.
    spawned: Instant,
    /// When Claude started; `None` until the guest logs it.
    claude_started: Option<Instant>,
}

impl<'a> Backstop<'a> {
    /// `None` for interactive terminals: they have no time limit.
    pub(super) fn new(ctx: &AppCtx, record: &TaskRecord, ws: &'a JobWorkspace) -> Option<Self> {
        let limit = Duration::from_secs(ctx.settings.get().timeout_s);
        (!record.interactive).then_some(Backstop { ws, limit, spawned: Instant::now(), claude_started: None })
    }

    /// When the server stops the VM, as known now (it moves once Claude starts).
    pub(super) fn deadline(&mut self) -> Instant {
        if self.claude_started.is_none()
            && let Some(at) = self.ws.job_log().iter().find_map(|l| claude_start_uptime(l))
        {
            // How long ago, from the guest's uptime now; unknown (no metrics yet): just now.
            let ago = self
                .ws
                .read_metrics()
                .map(|m| m.uptime_s as f64 - at)
                .filter(|s| s.is_finite() && *s > 0.0)
                .map_or(Duration::ZERO, Duration::from_secs_f64);
            self.claude_started = Some(Instant::now().checked_sub(ago).unwrap_or(self.spawned));
        }
        match self.claude_started {
            Some(start) => start + self.limit + TIMEOUT_GRACE,
            None => self.spawned + PREPARE_MAX + self.limit + TIMEOUT_GRACE,
        }
    }
}
