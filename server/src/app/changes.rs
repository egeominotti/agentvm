//! A counter bumped each time the task list changes in a way the dashboard shows (a state, what
//! Claude is doing, a task added or removed), not by telemetry. Dashboards wait on it instead of
//! polling: a browser slows a background tab's timers to once a minute, not its network events.

use std::sync::Arc;

use tokio::sync::watch;

#[derive(Clone)]
pub struct Changes(Arc<watch::Sender<u64>>);

impl Default for Changes {
    fn default() -> Self {
        Changes(Arc::new(watch::channel(0).0))
    }
}

impl Changes {
    pub fn bump(&self) {
        self.0.send_modify(|n| *n = n.wrapping_add(1));
    }

    /// Wakes on the next change; bursts of changes wake it once.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.0.subscribe()
    }
}
