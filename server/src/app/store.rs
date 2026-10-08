//! Task repository: sole owner of their state (in memory for the MVP).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use tokio::sync::watch;

use super::events::{EventLog, StreamItem};
// The record type lives in its own module; re-exported where callers have always found it.
pub use super::record::{HISTORY, TaskRecord};
use crate::domain::ids::TaskId;
use crate::domain::metrics::{ForwardedPort, VmMetrics};
use crate::domain::task::{InvalidTransition, TaskEvent, TaskState, transition};
use crate::domain::usage::AgentUsage;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("task not found")]
    NotFound,
    #[error(transparent)]
    Invalid(#[from] InvalidTransition),
}

struct Entry {
    record: TaskRecord,
    log: Arc<EventLog>,
    stop: watch::Sender<bool>,
}

/// Cheap to clone: clones share the same tasks.
#[derive(Default, Clone)]
pub struct Store {
    tasks: Arc<Mutex<HashMap<TaskId, Entry>>>,
    /// When set, every task is mirrored to `<dir>/<id>/record.json` to survive restarts.
    dir: Option<PathBuf>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn persistent(dir: PathBuf) -> Self {
        Store { tasks: Arc::default(), dir: Some(dir) }
    }

    /// Another handle on the same tasks (for callbacks that outlive a borrow).
    pub fn clone_handle(&self) -> Store {
        self.clone()
    }

    /// Tasks saved by a previous run.
    /// Every task saved in `dir`, and the job folders whose record exists but cannot be read
    /// (their VMs must be left alone, not treated as orphans).
    pub fn load_all(dir: &Path) -> (Vec<TaskRecord>, Vec<String>) {
        let (mut records, mut unreadable) = (Vec::new(), Vec::new());
        let Ok(entries) = std::fs::read_dir(dir) else { return (records, unreadable) };
        for e in entries.flatten() {
            let Ok(bytes) = std::fs::read(e.path().join("record.json")) else { continue };
            match serde_json::from_slice(&bytes) {
                Ok(r) => records.push(r),
                Err(_) => unreadable.push(e.file_name().to_string_lossy().into_owned()),
            }
        }
        unreadable.sort();
        (records, unreadable)
    }

    pub fn load(dir: &Path) -> Vec<TaskRecord> {
        Self::load_all(dir).0
    }

    fn persist(&self, record: &TaskRecord) {
        if let Some(dir) = &self.dir {
            let _ = crate::adapters::records::save(&dir.join(record.id.as_str()).join("record.json"), record);
        }
    }

    pub fn insert(&self, record: TaskRecord) {
        let log = Arc::new(EventLog::new());
        log.push(StreamItem::State(record.state.clone()));
        self.persist(&record);
        let entry = Entry { record, log, stop: watch::channel(false).0 };
        self.tasks.lock().unwrap().insert(entry.record.id.clone(), entry);
    }

    pub fn apply(&self, id: &TaskId, event: TaskEvent) -> Result<TaskState, StoreError> {
        let mut tasks = self.tasks.lock().unwrap();
        let entry = tasks.get_mut(id).ok_or(StoreError::NotFound)?;
        let next = transition(&entry.record.state, &event)?;
        if next != entry.record.state {
            if next.is_terminal() {
                entry.record.finished_at = Some(SystemTime::now());
            }
            entry.record.state = next.clone();
            entry.log.push(StreamItem::State(next.clone()));
            self.persist(&entry.record);
        }
        Ok(next)
    }

    /// Signals the stop to the supervisor and updates the state (a queued task stops immediately).
    pub fn request_stop(&self, id: &TaskId) -> Result<TaskState, StoreError> {
        let next = self.apply(id, TaskEvent::StopRequested)?;
        if let Some(e) = self.tasks.lock().unwrap().get(id) {
            e.stop.send_replace(true);
        }
        Ok(next)
    }

    pub fn set_activity(&self, id: &TaskId, activity: Option<String>) {
        if let Some(e) = self.tasks.lock().unwrap().get_mut(id) {
            e.record.activity = activity;
        }
    }

    pub fn record_metrics(&self, id: &TaskId, m: VmMetrics) {
        if let Some(e) = self.tasks.lock().unwrap().get_mut(id) {
            e.record.push_metrics(m);
        }
    }

    /// Memory reserved by the VMs that hold a slot.
    pub fn committed_memory_mb(&self) -> u64 {
        self.tasks.lock().unwrap().values().filter(|e| e.record.holds_vm()).map(|e| e.record.memory_mb).sum()
    }

    /// Saved to disk only when it changes.
    pub fn set_auto_snapshot_min(&self, id: &TaskId, minutes: Option<u32>) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(e) = tasks.get_mut(id) {
            e.record.auto_snapshot_min = minutes;
            let record = e.record.clone();
            drop(tasks);
            self.persist(&record);
        }
    }

    pub fn set_usage(&self, id: &TaskId, usage: AgentUsage) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(e) = tasks.get_mut(id)
            && e.record.usage.as_ref() != Some(&usage)
        {
            e.record.usage = Some(usage);
            let record = e.record.clone();
            drop(tasks);
            self.persist(&record);
        }
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
            e.record.ready = ready;
        }
    }

    pub fn set_ports(&self, id: &TaskId, ports: Vec<ForwardedPort>) {
        if let Some(e) = self.tasks.lock().unwrap().get_mut(id) {
            e.record.ports = ports;
        }
    }

    /// Forgets a task (the caller deletes its files).
    pub fn remove(&self, id: &TaskId) -> Option<TaskRecord> {
        self.tasks.lock().unwrap().remove(id).map(|e| e.record)
    }

    pub fn running_count(&self) -> usize {
        self.tasks.lock().unwrap().values().filter(|e| e.record.holds_vm()).count()
    }

    pub fn stop_signal(&self, id: &TaskId) -> Option<watch::Receiver<bool>> {
        self.tasks.lock().unwrap().get(id).map(|e| e.stop.subscribe())
    }

    pub fn get(&self, id: &TaskId) -> Option<TaskRecord> {
        self.tasks.lock().unwrap().get(id).map(|e| e.record.clone())
    }

    /// Most recent first.
    /// The first task matching `pred`, without cloning the others (proxy lookups run per request).
    pub fn find_id(&self, pred: impl Fn(&TaskRecord) -> bool) -> Option<TaskId> {
        self.tasks.lock().unwrap().values().find(|e| pred(&e.record)).map(|e| e.record.id.clone())
    }

    pub fn list(&self) -> Vec<TaskRecord> {
        let mut all: Vec<_> = self.tasks.lock().unwrap().values().map(|e| e.record.clone()).collect();
        all.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        all
    }

    pub fn log(&self, id: &TaskId) -> Option<Arc<EventLog>> {
        self.tasks.lock().unwrap().get(id).map(|e| e.log.clone())
    }
}
