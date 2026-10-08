//! Task repository: sole owner of their state (in memory for the MVP).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use tokio::sync::watch;

use super::events::{EventLog, StreamItem};
use crate::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};
use crate::domain::metrics::VmMetrics;
use crate::domain::settings::Model;
use crate::domain::snapshot::SnapshotId;
use crate::domain::task::{InvalidTransition, TaskEvent, TaskState, transition};
use crate::domain::usage::AgentUsage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskRecord {
    pub id: TaskId,
    pub repo: RepoPath,
    /// Absent for a terminal opened without an initial task.
    pub prompt: Option<Prompt>,
    pub base_sha: CommitSha,
    pub interactive: bool,
    pub state: TaskState,
    /// Last activity reported by the Claude Code hooks (`working`, `waiting`).
    #[serde(skip)]
    pub activity: Option<String>,
    pub model: Model,
    /// Claude Code version installed at boot instead of the image's one.
    pub claude_version: Option<String>,
    /// Snapshot this machine was restored from.
    pub restore_from: Option<SnapshotId>,
    /// Display name when there is no first task (e.g. "Restored: …").
    #[serde(default)]
    pub label: Option<String>,
    /// Resources of this VM (from the launch, or the settings at launch time).
    pub cpus: u32,
    pub memory_mb: u64,
    /// Latest telemetry sample and the last `HISTORY` CPU and memory percentages.
    /// Claude's cost and tokens (kept across restarts).
    #[serde(default)]
    pub usage: Option<AgentUsage>,
    /// Boot timeline: host steps (`host: …`) then the guest job's own log lines.
    #[serde(skip)]
    pub boot_log: Vec<String>,
    /// The agent's terminal is up (interactive) or the job is running (automatic).
    #[serde(skip)]
    pub ready: bool,
    /// VM ports reachable from the Mac right now.
    #[serde(skip)]
    pub ports: Vec<crate::domain::metrics::ForwardedPort>,
    #[serde(skip)]
    pub metrics: Option<VmMetrics>,
    #[serde(skip)]
    pub cpu_history: VecDeque<f32>,
    #[serde(skip)]
    pub mem_history: VecDeque<f32>,
    pub created_at: SystemTime,
    pub finished_at: Option<SystemTime>,
}

/// One minute of samples at one per second.
pub const HISTORY: usize = 60;

impl TaskRecord {
    pub fn new(id: TaskId, repo: RepoPath, prompt: Option<Prompt>, base_sha: CommitSha, interactive: bool) -> Self {
        TaskRecord {
            id,
            repo,
            prompt,
            base_sha,
            interactive,
            state: TaskState::Queued,
            activity: None,
            model: Model::default_choice(),
            claude_version: None,
            restore_from: None,
            label: None,
            cpus: 0,
            memory_mb: 0,
            usage: None,
            boot_log: Vec::new(),
            ready: false,
            ports: Vec::new(),
            metrics: None,
            cpu_history: VecDeque::with_capacity(HISTORY),
            mem_history: VecDeque::with_capacity(HISTORY),
            created_at: SystemTime::now(),
            finished_at: None,
        }
    }

    pub fn with_model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }

    /// The task holds a VM slot (queued and finished tasks do not).
    pub fn holds_vm(&self) -> bool {
        matches!(self.state, TaskState::Preparing | TaskState::Booting | TaskState::Running | TaskState::Collecting)
    }
}

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
    pub fn load(dir: &Path) -> Vec<TaskRecord> {
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        entries
            .flatten()
            .filter_map(|e| std::fs::read(e.path().join("record.json")).ok())
            .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
            .collect()
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
            let r = &mut e.record;
            for (history, value) in [(&mut r.cpu_history, m.cpu_pct as f32), (&mut r.mem_history, m.mem_pct() as f32)] {
                if history.len() == HISTORY {
                    history.pop_front();
                }
                history.push_back(value);
            }
            r.metrics = Some(m);
        }
    }

    /// Memory reserved by the VMs that hold a slot.
    pub fn committed_memory_mb(&self) -> u64 {
        self.tasks.lock().unwrap().values().filter(|e| e.record.holds_vm()).map(|e| e.record.memory_mb).sum()
    }

    /// Saved to disk only when it changes.
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

    pub fn set_ports(&self, id: &TaskId, ports: Vec<crate::domain::metrics::ForwardedPort>) {
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
    pub fn list(&self) -> Vec<TaskRecord> {
        let mut all: Vec<_> = self.tasks.lock().unwrap().values().map(|e| e.record.clone()).collect();
        all.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        all
    }

    pub fn log(&self, id: &TaskId) -> Option<Arc<EventLog>> {
        self.tasks.lock().unwrap().get(id).map(|e| e.log.clone())
    }
}
