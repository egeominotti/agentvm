//! History + publishing of a task's events, for multiple concurrent clients.
mod persistence;

use std::sync::Mutex;

use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum StreamItem {
    State(TaskState),
    Agent(AgentEvent),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct OutputItem {
    pub seq: u64,
    pub item: StreamItem,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct OutputPage {
    pub items: Vec<OutputItem>,
    pub next_cursor: u64,
    pub more: bool,
    pub dropped: u64,
}

pub type Seq = u64;
pub type Snapshot = (Vec<(Seq, StreamItem)>, broadcast::Receiver<(Seq, StreamItem)>);

pub struct EventLog {
    history: Mutex<std::collections::VecDeque<(Seq, StreamItem)>>,
    next: std::sync::atomic::AtomicU64,
    live: broadcast::Sender<(Seq, StreamItem)>,
    file: Option<std::path::PathBuf>,
    import_pending: std::sync::atomic::AtomicBool,
    source: Mutex<Option<crate::adapters::tail::Cursor>>,
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
            file: None,
            import_pending: std::sync::atomic::AtomicBool::new(true),
            source: Mutex::new(None),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.history.lock().unwrap().is_empty()
    }

    /// Only legacy finished jobs without normalized events need the raw stream imported.
    pub fn begin_legacy_import(&self) -> bool {
        self.import_pending.swap(false, std::sync::atomic::Ordering::Relaxed)
            && !self.history.lock().unwrap().iter().any(|(_, item)| matches!(item, StreamItem::Agent(_)))
    }

    pub fn import_legacy(&self, mut events: Vec<AgentEvent>, truncated_prefix: bool) {
        if events.is_empty() {
            if !truncated_prefix {
                return;
            }
            events.push(AgentEvent::Unparsed { raw: "(agentvm omitted a legacy output prefix)".into() });
        }
        let state = self.history.lock().unwrap().back().map(|(_, item)| item.clone());
        if truncated_prefix {
            self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        for event in events {
            self.push(StreamItem::Agent(event));
        }
        if let Some(state) = state {
            self.push(state);
        }
        self.compact(FINISHED_HISTORY);
    }

    /// Cursor is the last consumed sequence. Missing IDs are reported rather than silently lost.
    pub fn page(&self, cursor: u64, limit: usize) -> OutputPage {
        let history = self.history.lock().unwrap();
        let mut available = history.iter().filter(|(seq, _)| *seq > cursor);
        let items: Vec<_> = available
            .by_ref()
            .take(limit.clamp(1, 500))
            .map(|(seq, item)| OutputItem { seq: *seq, item: item.clone() })
            .collect();
        let more = available.next().is_some();
        let mut next_cursor = cursor;
        let mut dropped = 0;
        for item in &items {
            dropped += item.seq.saturating_sub(next_cursor.saturating_add(1));
            next_cursor = item.seq;
        }
        if items.is_empty() {
            let latest = self.next.load(std::sync::atomic::Ordering::Relaxed).saturating_sub(1);
            dropped = latest.saturating_sub(cursor);
            next_cursor = cursor.max(latest);
        }
        OutputPage { items, next_cursor, more, dropped }
    }

    pub fn source_cursor(&self) -> Option<crate::adapters::tail::Cursor> {
        self.source.lock().unwrap().clone()
    }

    pub fn push(&self, item: StreamItem) {
        self.publish(vec![item], None);
    }

    /// Events and consumed bytes form one complete JSON journal line, so replay cannot duplicate a partially written batch.
    pub fn publish_agent_line(&self, events: Vec<AgentEvent>, cursor: crate::adapters::tail::Cursor) {
        self.publish(events.into_iter().map(StreamItem::Agent).collect(), Some(cursor));
    }

    fn publish(&self, items: Vec<StreamItem>, cursor: Option<crate::adapters::tail::Cursor>) {
        let mut history = self.history.lock().unwrap();
        if let Some(cursor) = cursor {
            *self.source.lock().unwrap() = Some(cursor);
        }
        let items: Vec<_> = items
            .into_iter()
            .map(|item| OutputItem { seq: self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed), item })
            .collect();
        self.persist_batch(items.clone(), false);
        for OutputItem { seq, item } in items {
            if history.len() == HISTORY_MAX {
                history.pop_front();
            }
            history.push_back((seq, item.clone()));
            let _ = self.live.send((seq, item));
        }
    }

    pub fn compact(&self, keep: usize) {
        let mut history = self.history.lock().unwrap();
        let drop = history.len().saturating_sub(keep);
        history.drain(..drop);
        history.shrink_to_fit();
        self.persist_batch(
            history.iter().map(|(seq, item)| OutputItem { seq: *seq, item: item.clone() }).collect(),
            true,
        );
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
