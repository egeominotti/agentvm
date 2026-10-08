//! The server's time limit for an automatic task. The guest enforces the limit itself and saves
//! the work; this is the backstop if it cannot. It counts from when Claude starts, as the guest
//! does: boot and `setup.sh` can take long.

use std::time::{Duration, Instant};

use crate::adapters::jobdir::JobWorkspace;
use crate::app::context::AppCtx;
use crate::app::record::TaskRecord;
use crate::domain::outcome::backstop;

/// What the guest job logs when Claude starts.
const CLAUDE_START: &str = "] claude start";

pub(super) struct Backstop<'a> {
    ws: &'a JobWorkspace,
    limit: Duration,
    /// When this server started following the VM.
    spawned: Instant,
    /// When Claude started, counted from `spawned`; `None` until the guest logs it.
    claude_started: Option<Duration>,
}

impl<'a> Backstop<'a> {
    /// `None` for interactive terminals: they have no time limit.
    pub(super) fn new(ctx: &AppCtx, record: &TaskRecord, ws: &'a JobWorkspace) -> Option<Self> {
        let limit = Duration::from_secs(ctx.settings.get().timeout_s);
        let spawned = Instant::now();
        (!record.interactive).then_some(Backstop { ws, limit, spawned, claude_started: None })
    }

    /// When the server stops the VM, as known now (it moves once Claude starts).
    pub(super) fn deadline(&mut self) -> Instant {
        if self.claude_started.is_none() && self.ws.job_log().iter().any(|l| l.ends_with(CLAUDE_START)) {
            self.claude_started = Some(self.spawned.elapsed());
        }
        self.spawned + backstop(self.limit, self.claude_started)
    }
}
