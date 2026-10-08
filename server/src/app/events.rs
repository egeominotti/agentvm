//! History + publishing of a task's events, for multiple concurrent clients.

use std::sync::Mutex;

use futures::{Stream, StreamExt};
use serde::Serialize;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::domain::agent_event::AgentEvent;
use crate::domain::task::TaskState;

/// Slots are allocated up front for every task: 256 is plenty for live clients (~24 KB, not 386 KB).
const LIVE_CAPACITY: usize = 256;
/// Events kept for clients that connect later; older ones are dropped.
const HISTORY_MAX: usize = 10_000;
/// Events a finished task keeps: enough to read how it ended, cheap for hundreds of tasks.
pub const FINISHED_HISTORY: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum StreamItem {
    State(TaskState),
    Agent(AgentEvent),
}

pub type Seq = u64;
pub type Snapshot = (Vec<(Seq, StreamItem)>, broadcast::Receiver<(Seq, StreamItem)>);

pub struct EventLog {
    history: Mutex<std::collections::VecDeque<(Seq, StreamItem)>>,
    next: std::sync::atomic::AtomicU64,
    live: broadcast::Sender<(Seq, StreamItem)>,
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLog {
    pub fn new() -> Self {
        EventLog {
            history: Mutex::new(std::collections::VecDeque::new()),
            next: std::sync::atomic::AtomicU64::new(1),
            live: broadcast::channel(LIVE_CAPACITY).0,
        }
    }

    pub fn push(&self, item: StreamItem) {
        let mut history = self.history.lock().unwrap();
        let seq = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if history.len() == HISTORY_MAX {
            history.pop_front();
        }
        history.push_back((seq, item.clone()));
        // Sent under the lock: a subscriber sees each event either in the history or live, never both.
        let _ = self.live.send((seq, item));
    }

    /// Keeps only the newest `keep` events (a finished task does not need them all).
    pub fn compact(&self, keep: usize) {
        let mut history = self.history.lock().unwrap();
        let drop = history.len().saturating_sub(keep);
        history.drain(..drop);
        history.shrink_to_fit();
    }

    pub fn subscribe(&self) -> Snapshot {
        let history = self.history.lock().unwrap();
        (history.iter().cloned().collect(), self.live.subscribe())
    }

    /// Full history followed by live events.
    pub fn stream(&self) -> impl Stream<Item = (Seq, StreamItem)> + Send + use<> {
        let (history, rx) = self.subscribe();
        let last = history.last().map_or(0, |(s, _)| *s);
        futures::stream::iter(history)
            .chain(BroadcastStream::new(rx).filter_map(move |r| async move { r.ok().filter(|(s, _)| *s > last) }))
    }
}
