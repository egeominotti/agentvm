//! A task's `record.json`, written outside the store's lock but never out of order: each
//! change gets a version, and a write older than one already on disk is dropped. A write that
//! fails (disk full, a folder in the way) is kept and tried again; a task deleted from the list
//! is never written again.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::record::TaskRecord;

#[derive(Default)]
struct State {
    /// The version on disk.
    written: u64,
    /// The newest version that could not be written yet.
    unsaved: Option<(TaskRecord, u64)>,
    /// The task was deleted: a late write must not bring its folder back.
    forgotten: bool,
}

/// Cheap to clone: clones share the same state.
#[derive(Clone, Default)]
pub(super) struct RecordFile {
    /// `None` for a store kept in memory only.
    path: Option<PathBuf>,
    state: Arc<Mutex<State>>,
}

impl RecordFile {
    pub(super) fn new(path: Option<PathBuf>) -> Self {
        RecordFile { path, state: Arc::default() }
    }

    /// Writes `record` as of `version`, unless a newer version is already on disk. Writers of
    /// the same task wait for each other; other tasks and the store are never blocked.
    pub(super) fn write(&self, record: &TaskRecord, version: u64) {
        let Some(path) = &self.path else { return };
        let mut state = self.state.lock().unwrap();
        if state.forgotten || version <= state.written {
            return;
        }
        match crate::adapters::records::save(path, record) {
            Ok(()) => {
                state.written = version;
                state.unsaved = None;
            }
            Err(e) => {
                tracing::warn!(task = %record.id, error = %e, "task record not saved: trying again");
                state.unsaved = Some((record.clone(), version));
            }
        }
    }

    /// Writes the version that could not be written before, if any.
    pub(super) fn retry(&self) {
        let pending = self.state.lock().unwrap().unsaved.take();
        if let Some((record, version)) = pending {
            self.write(&record, version);
        }
    }

    /// The task is gone: nothing is written for it any more.
    pub(super) fn forget(&self) {
        let mut state = self.state.lock().unwrap();
        state.forgotten = true;
        state.unsaved = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> TaskRecord {
        use crate::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".git")).unwrap();
        TaskRecord::new(
            TaskId::generate(std::time::SystemTime::now(), &[1, 2]),
            RepoPath::new(repo.path().to_path_buf()).unwrap(),
            Some(Prompt::new("x".into()).unwrap()),
            CommitSha::parse(&"b".repeat(40)).unwrap(),
            false,
        )
    }

    /// A write still in flight when the task is deleted cannot bring its folder back (it would
    /// reappear in the list after a restart).
    #[test]
    fn a_forgotten_record_is_never_written_again() {
        let jobs = tempfile::tempdir().unwrap();
        let path = jobs.path().join("task").join("record.json");
        let file = RecordFile::new(Some(path.clone()));
        file.write(&record(), 1);
        file.forget();
        std::fs::remove_dir_all(jobs.path().join("task")).unwrap();
        file.write(&record(), 2);
        file.retry();
        assert!(!path.exists(), "the deleted task's folder came back");
    }
}
