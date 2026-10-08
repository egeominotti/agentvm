//! A task's `record.json`, written outside the store's lock but never out of order: each
//! change gets a version, and a write older than one already on disk is dropped.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::record::TaskRecord;

/// Cheap to clone: clones share the version last written.
#[derive(Clone, Default)]
pub(super) struct RecordFile {
    /// `None` for a store kept in memory only.
    path: Option<PathBuf>,
    written: Arc<Mutex<u64>>,
}

impl RecordFile {
    pub(super) fn new(path: Option<PathBuf>) -> Self {
        RecordFile { path, written: Arc::default() }
    }

    /// Writes `record` as of `version`, unless a newer version is already on disk. Writers of
    /// the same task wait for each other; other tasks and the store are never blocked.
    pub(super) fn write(&self, record: &TaskRecord, version: u64) {
        let Some(path) = &self.path else { return };
        let mut written = self.written.lock().unwrap();
        if version > *written && crate::adapters::records::save(path, record).is_ok() {
            *written = version;
        }
    }
}
