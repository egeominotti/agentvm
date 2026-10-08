//! A saved copy of a VM's disk that can be restored into a new VM.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::ids::TaskId;

/// `snap-YYYYMMDD-HHMMSS-xxxx`: safe as a folder name, sortable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SnapshotId(String);

impl SnapshotId {
    pub fn generate(now: SystemTime, rand: [u8; 2]) -> Self {
        SnapshotId(format!("snap-{}", TaskId::generate(now, rand)))
    }

    pub fn parse(s: &str) -> Option<Self> {
        let rest = s.strip_prefix("snap-")?;
        TaskId::parse(rest).map(|_| SnapshotId(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub id: SnapshotId,
    pub name: String,
    /// The task whose VM was saved.
    pub source_task: String,
    pub repo: String,
    pub base_sha: String,
    pub model: String,
    pub claude_version: Option<String>,
    pub created_at: f64,
    /// Size of the disk image (copy-on-write: blocks shared with the golden image count too).
    pub size_mb: u64,
    /// Resources of the source VM, reused on restore (0 = use the settings).
    #[serde(default)]
    pub cpus: u32,
    #[serde(default)]
    pub memory_mb: u64,
}
