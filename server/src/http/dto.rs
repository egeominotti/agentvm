//! JSON shapes exposed by the API.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app::store::TaskRecord;
use crate::domain::metrics::VmMetrics;
use crate::domain::settings::{HostLimits, Model, Settings};
use crate::domain::task::TaskState;

#[derive(Deserialize)]
pub struct CreateTask {
    pub repo_path: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub base_ref: Option<String>,
    /// The branch to start from (the default when absent); wins over `base_ref`.
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub model: Option<Model>,
    #[serde(default)]
    pub claude_version: Option<String>,
    #[serde(default)]
    pub cpus: Option<u32>,
    #[serde(default)]
    pub memory_mb: Option<u64>,
    /// Join the user's tailnet (`null`: as the settings say).
    #[serde(default)]
    pub tailscale: Option<bool>,
}

#[derive(Deserialize, Default)]
pub struct SnapshotRequest {
    #[serde(default)]
    pub name: Option<String>,
}

/// `{"every_min": 15}` for this machine only, `{"every_min": null}` to follow the settings.
#[derive(Deserialize)]
pub struct AutoSnapshotInterval {
    pub every_min: Option<u32>,
}

#[derive(Deserialize)]
pub struct S3Update {
    pub config: crate::domain::s3::S3Config,
    #[serde(default)]
    pub secret: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SettingsView {
    pub settings: Settings,
    pub limits: HostLimits,
    /// VMs that fit in RAM with the current memory per VM.
    pub recommended_max_vms: usize,
}

#[derive(Deserialize)]
pub struct TokenUpdate {
    pub token: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Saved {
    pub commits: u32,
    /// Where the work landed: `agent/<id>`, or `agent/<id>-vm` when that branch has your own commits.
    pub branch: String,
}

#[derive(Deserialize)]
pub struct PtyQuery {
    #[serde(default = "default_session")]
    pub session: String,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
    /// Read-only preview that does not resize the session.
    #[serde(default)]
    pub view: bool,
}

fn default_session() -> String {
    "claude".into()
}
fn default_cols() -> u16 {
    120
}
fn default_rows() -> u16 {
    36
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Created {
    pub id: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Status {
    pub golden: bool,
    pub token: bool,
    pub token_hint: Option<String>,
    pub concurrency: usize,
    pub running: usize,
    pub host: HostLimits,
    /// Memory promised to running VMs.
    pub ram_committed_mb: u64,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct TaskDto {
    pub id: String,
    pub repo: String,
    pub prompt: String,
    pub base_sha: String,
    pub branch: String,
    pub interactive: bool,
    pub activity: Option<String>,
    pub model: Model,
    pub claude_version: Option<String>,
    pub cpus: u32,
    pub memory_mb: u64,
    pub label: Option<String>,
    /// This machine's own interval for automatic snapshots (`null`: the settings').
    pub auto_snapshot_min: Option<u32>,
    pub ports: Vec<crate::domain::metrics::ForwardedPort>,
    /// Asked to join the user's tailnet.
    pub tailscale: bool,
    /// On the tailnet: its name and addresses, or why it did not join (`null`: not yet known).
    pub tailnet: Option<crate::domain::tailscale::Tailnet>,
    pub boot_log: Vec<String>,
    pub ready: bool,
    pub usage: Option<crate::domain::usage::AgentUsage>,
    pub metrics: Option<VmMetrics>,
    pub cpu_history: Vec<f32>,
    pub mem_history: Vec<f32>,
    /// Seconds since the last new sample (`null`: none yet); over a few seconds, the numbers are stale.
    pub metrics_age_s: Option<f64>,
    /// What the balloon lets the VM keep now (`null`: not known yet).
    pub memory_limit_mb: Option<u64>,
    pub status: TaskState,
    pub created_at: f64,
    pub finished_at: Option<f64>,
}

impl From<TaskRecord> for TaskDto {
    fn from(r: TaskRecord) -> Self {
        TaskDto {
            branch: r.branch(),
            id: r.id.to_string(),
            repo: r.repo.as_path().display().to_string(),
            prompt: r.prompt.map(|p| p.as_str().to_owned()).unwrap_or_default(),
            base_sha: r.base_sha.as_str().to_owned(),
            interactive: r.interactive,
            activity: r.activity,
            model: r.model,
            claude_version: r.claude_version,
            cpus: r.cpus,
            memory_mb: r.memory_mb,
            auto_snapshot_min: r.auto_snapshot_min,
            label: r.label,
            ports: r.ports,
            tailscale: r.tailscale,
            tailnet: r.tailnet,
            boot_log: r.boot_log,
            ready: r.ready,
            usage: r.usage,
            metrics: r.metrics,
            cpu_history: r.cpu_history.into(),
            mem_history: r.mem_history.into(),
            metrics_age_s: r.metrics_at.map(|at| (now_s() - at).max(0.0)),
            memory_limit_mb: r.memory_limit_mb,
            status: r.state,
            created_at: unix(r.created_at),
            finished_at: r.finished_at.map(unix),
        }
    }
}

fn unix(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn now_s() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}
