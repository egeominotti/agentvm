//! Task repository: sole owner of their state (in memory for the MVP).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use tokio::sync::watch;

use super::events::{EventLog, StreamItem};
use super::record_file::RecordFile;
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
    /// Bumped by every change that goes to disk.
    version: u64,
    file: RecordFile,
}

/// A change to write once the store's lock is released.
type Pending = Option<(TaskRecord, u64, RecordFile)>;

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

    /// Runs `change` under the lock; when it reports the record changed, the new record is
    /// written to disk after the lock is released (disk I/O never blocks the store).
    fn update<R>(&self, id: &TaskId, change: impl FnOnce(&mut TaskRecord) -> (R, bool)) -> Option<R> {
        let (out, pending): (R, Pending) = {
            let mut tasks = self.tasks.lock().unwrap();
            let e = tasks.get_mut(id)?;
            let (out, changed) = change(&mut e.record);
            let pending = changed.then(|| {
                e.version += 1;
                (e.record.clone(), e.version, e.file.clone())
            });
            (out, pending)
        };
        if let Some((record, version, file)) = pending {
            file.write(&record, version);
        }
        Some(out)
    }

    /// Adds or replaces a task (reloading the saved ones after a restart).
    pub fn insert(&self, record: TaskRecord) {
        let entry = self.entry(record);
        self.tasks.lock().unwrap().insert(entry.record.id.clone(), entry);
    }

    /// Adds a new task, unless its id is already taken: it is handed back untouched.
    pub fn try_insert(&self, record: TaskRecord) -> Result<(), Box<TaskRecord>> {
        let mut tasks = self.tasks.lock().unwrap();
        if tasks.contains_key(&record.id) {
            return Err(Box::new(record));
        }
        let entry = self.entry(record);
        tasks.insert(entry.record.id.clone(), entry);
        Ok(())
    }

    fn entry(&self, record: TaskRecord) -> Entry {
        let log = Arc::new(EventLog::new());
        log.push(StreamItem::State(record.state.clone()));
        let file = RecordFile::new(self.dir.as_ref().map(|d| d.join(record.id.as_str()).join("record.json")));
        file.write(&record, 1);
        Entry { record, log, stop: watch::channel(false).0, version: 1, file }
    }

    pub fn apply(&self, id: &TaskId, event: TaskEvent) -> Result<TaskState, StoreError> {
        let log = self.log(id).ok_or(StoreError::NotFound)?;
        let result = self.update(id, |record| match transition(&record.state, &event) {
            Err(e) => (Err(e.into()), false),
            Ok(next) if next == record.state => (Ok(next), false),
            Ok(next) => {
                let from = record.state.kind();
                match &next {
                    TaskState::Failed { reason } => {
                        tracing::warn!(task = %id, from, to = next.kind(), reason = %reason, "state changed")
                    }
                    _ => tracing::info!(task = %id, from, to = next.kind(), "state changed"),
                }
                if next.is_terminal() {
                    record.finished_at = Some(SystemTime::now());
                }
                record.state = next.clone();
                log.push(StreamItem::State(next.clone()));
                if next.is_terminal() {
                    log.compact(super::events::FINISHED_HISTORY);
                }
                (Ok(next), true)
            }
        });
        result.unwrap_or(Err(StoreError::NotFound))
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
