//! History + publishing of a task's events, for multiple concurrent clients.

use std::sync::Mutex;

use futures::{Stream, StreamExt};
use serde::Serialize;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::domain::agent_event::AgentEvent;
use crate::domain::task::TaskState;

const LIVE_CAPACITY: usize = 4096;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum StreamItem {
    State(TaskState),
    Agent(AgentEvent),
}

pub type Seq = u64;
pub type Snapshot = (Vec<(Seq, StreamItem)>, broadcast::Receiver<(Seq, StreamItem)>);

pub struct EventLog {
    history: Mutex<Vec<(Seq, StreamItem)>>,
    live: broadcast::Sender<(Seq, StreamItem)>,
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLog {
    pub fn new() -> Self {
        EventLog { history: Mutex::new(Vec::new()), live: broadcast::channel(LIVE_CAPACITY).0 }
    }

    pub fn push(&self, item: StreamItem) {
        let mut history = self.history.lock().unwrap();
        let seq = history.len() as Seq + 1;
        history.push((seq, item.clone()));
        // Sent under the lock: a subscriber sees each event either in the history or live, never both.
        let _ = self.live.send((seq, item));
    }

    pub fn subscribe(&self) -> Snapshot {
        let history = self.history.lock().unwrap();
        (history.clone(), self.live.subscribe())
    }

    /// Full history followed by live events.
    pub fn stream(&self) -> impl Stream<Item = (Seq, StreamItem)> + Send + use<> {
        let (history, rx) = self.subscribe();
        let last = history.last().map_or(0, |(s, _)| *s);
        futures::stream::iter(history).chain(
            BroadcastStream::new(rx).filter_map(move |r| async move { r.ok().filter(|(s, _)| *s > last) }),
        )
    }
}
