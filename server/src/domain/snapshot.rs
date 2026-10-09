//! A saved copy of a VM's disk that can be restored into a new VM.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::ids::TaskId;

/// `snap-YYYYMMDD-HHMMSS-xxxx`: safe as a folder name, sortable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, type = "string"))]
pub struct SnapshotId(String);

impl SnapshotId {
    pub fn generate(now: SystemTime, rand: &[u8]) -> Self {
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
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
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
    /// Taken by the schedule (or before a close): pruned to the newest `keep` per VM.
    #[serde(default)]
    pub auto: bool,
}

/// Automatic snapshots of running terminals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AutoSnapshots {
    /// Minutes between snapshots; 0 turns the schedule off.
    pub every_min: u32,
    /// Automatic snapshots kept per VM (manual ones are never pruned).
    pub keep: u32,
    /// One more snapshot just before a VM is closed.
    pub before_close: bool,
}

impl Default for AutoSnapshots {
    fn default() -> Self {
        AutoSnapshots { every_min: 30, keep: 4, before_close: true }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AutoSnapshotsError {
    #[error("automatic snapshots run every 1, 5, 10, 15, 30, 60, 120 or 240 minutes, or never (0)")]
    Interval,
    #[error("keep between 1 and 50 automatic snapshots per VM")]
    Keep,
}

impl AutoSnapshots {
    pub const INTERVALS: [u32; 9] = [0, 1, 5, 10, 15, 30, 60, 120, 240];

    pub fn validate(&self) -> Result<(), AutoSnapshotsError> {
        if !Self::INTERVALS.contains(&self.every_min) {
            return Err(AutoSnapshotsError::Interval);
        }
        if !(1..=50).contains(&self.keep) {
            return Err(AutoSnapshotsError::Keep);
        }
        Ok(())
    }

    /// Times are seconds since the epoch. The first one is counted from the VM's start.
    pub fn due(&self, last_auto: Option<f64>, started_at: f64, now: f64) -> bool {
        self.every_min > 0 && now - last_auto.unwrap_or(started_at) >= f64::from(self.every_min) * 60.0
    }

    /// The automatic snapshots of `task` beyond the newest `keep`.
    pub fn to_prune(&self, all: &[SnapshotMeta], task: &str) -> Vec<SnapshotId> {
        let mut autos: Vec<&SnapshotMeta> = all.iter().filter(|s| s.auto && s.source_task == task).collect();
        autos.sort_by(|a, b| b.created_at.total_cmp(&a.created_at));
        autos.into_iter().skip(self.keep as usize).map(|s| s.id.clone()).collect()
    }
}
