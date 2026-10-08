//! Repository dei task: unico proprietario del loro stato (in memoria per l'MVP).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use tokio::sync::watch;

use super::events::{EventLog, StreamItem};
use crate::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};
use crate::domain::task::{InvalidTransition, TaskEvent, TaskState, transition};

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: TaskId,
    pub repo: RepoPath,
    pub prompt: Prompt,
    pub base_sha: CommitSha,
    pub state: TaskState,
    pub created_at: SystemTime,
    pub finished_at: Option<SystemTime>,
}

impl TaskRecord {
    pub fn new(id: TaskId, repo: RepoPath, prompt: Prompt, base_sha: CommitSha) -> Self {
        TaskRecord {
            id,
            repo,
            prompt,
            base_sha,
            state: TaskState::Queued,
            created_at: SystemTime::now(),
            finished_at: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("task inesistente")]
    NotFound,
    #[error(transparent)]
    Invalid(#[from] InvalidTransition),
}

struct Entry {
    record: TaskRecord,
    log: Arc<EventLog>,
    stop: watch::Sender<bool>,
}

#[derive(Default)]
pub struct Store {
    tasks: Mutex<HashMap<TaskId, Entry>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, record: TaskRecord) {
        let log = Arc::new(EventLog::new());
        log.push(StreamItem::State(record.state.clone()));
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
        }
        Ok(next)
    }

    /// Segnala lo stop al supervisor e aggiorna lo stato (un task in coda si ferma subito).
    pub fn request_stop(&self, id: &TaskId) -> Result<TaskState, StoreError> {
        let next = self.apply(id, TaskEvent::StopRequested)?;
        if let Some(e) = self.tasks.lock().unwrap().get(id) {
            e.stop.send_replace(true);
        }
        Ok(next)
    }

    pub fn stop_signal(&self, id: &TaskId) -> Option<watch::Receiver<bool>> {
        self.tasks.lock().unwrap().get(id).map(|e| e.stop.subscribe())
    }

    pub fn get(&self, id: &TaskId) -> Option<TaskRecord> {
        self.tasks.lock().unwrap().get(id).map(|e| e.record.clone())
    }

    /// Più recenti per primi.
    pub fn list(&self) -> Vec<TaskRecord> {
        let mut all: Vec<_> = self.tasks.lock().unwrap().values().map(|e| e.record.clone()).collect();
        all.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        all
    }

    pub fn log(&self, id: &TaskId) -> Option<Arc<EventLog>> {
        self.tasks.lock().unwrap().get(id).map(|e| e.log.clone())
    }
}
