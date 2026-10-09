//! One task as the store keeps it: what is saved across restarts and what lives only in memory.

use std::collections::VecDeque;
use std::time::SystemTime;

use crate::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};
use crate::domain::metrics::{ForwardedPort, VmMetrics};
use crate::domain::settings::Model;
use crate::domain::snapshot::SnapshotId;
use crate::domain::task::{StateAt, TaskState};
use crate::domain::telemetry::TelemetrySample;
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
    #[serde(default)]
    pub model: Model,
    /// Claude Code version installed at boot instead of the image's one.
    #[serde(default)]
    pub claude_version: Option<String>,
    /// Snapshot this machine was restored from.
    #[serde(default)]
    pub restore_from: Option<SnapshotId>,
    /// Display name when there is no first task (e.g. "Restored: …").
    #[serde(default)]
    pub label: Option<String>,
    /// Minutes between automatic snapshots of this machine; `None` follows the settings.
    #[serde(default)]
    pub auto_snapshot_min: Option<u32>,
    /// Resources of this VM (from the launch, or the settings at launch time).
    #[serde(default)]
    pub cpus: u32,
    #[serde(default)]
    pub memory_mb: u64,
    /// Latest telemetry sample and the last `HISTORY` CPU and memory percentages.
    /// Claude's cost and tokens (kept across restarts).
    #[serde(default)]
    pub usage: Option<AgentUsage>,
    /// Force stop asked: kept across restarts, so a VM stopped by hand stays "Stopped" (and its
    /// disk is not kept as "Interrupted") whatever happens to the server meanwhile.
    #[serde(default)]
    pub stop_requested: bool,
    /// This VM joins the user's tailnet.
    #[serde(default)]
    pub tailscale: bool,
    /// What its guest said of the tailnet: joined (name, addresses) or why not.
    #[serde(skip)]
    pub tailnet: Option<crate::domain::tailscale::Tailnet>,
    /// Boot timeline: host steps (`host: …`) then the guest job's own log lines.
    #[serde(skip)]
    pub boot_log: Vec<String>,
    /// The agent's terminal is up (interactive) or the job is running (automatic).
    #[serde(skip)]
    pub ready: bool,
    /// VM ports reachable from the Mac right now.
    #[serde(skip)]
    pub ports: Vec<ForwardedPort>,
    #[serde(skip)]
    pub metrics: Option<VmMetrics>,
    #[serde(skip)]
    pub cpu_history: VecDeque<f32>,
    #[serde(skip)]
    pub mem_history: VecDeque<f32>,
    /// When the last new sample arrived (host clock): older than a few seconds means stale.
    #[serde(skip)]
    pub metrics_at: Option<f64>,
    /// What the balloon currently lets the VM keep.
    #[serde(skip)]
    pub memory_limit_mb: Option<u64>,
    /// The last 5 minutes of telemetry, one sample a second, for live charts.
    #[serde(skip)]
    pub live: VecDeque<TelemetrySample>,
    pub created_at: SystemTime,
    #[serde(default)]
    pub finished_at: Option<SystemTime>,
    /// Every state the task went through, with its time (empty for records from before).
    #[serde(default)]
    pub timeline: Vec<StateAt>,
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
            auto_snapshot_min: None,
            cpus: 0,
            memory_mb: 0,
            usage: None,
            stop_requested: false,
            tailscale: false,
            tailnet: None,
            boot_log: Vec::new(),
            ready: false,
            ports: Vec::new(),
            metrics: None,
            cpu_history: VecDeque::with_capacity(HISTORY),
            mem_history: VecDeque::with_capacity(HISTORY),
            metrics_at: None,
            memory_limit_mb: None,
            live: VecDeque::new(),
            created_at: SystemTime::now(),
            finished_at: None,
            timeline: vec![StateAt::now(&TaskState::Queued)],
        }
    }

    pub fn with_model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }

    /// The task holds a VM slot (queued and finished tasks do not).
    /// Where the task's work is: the branch it landed on once done (`agent/<id>-vm` when the
    /// user had commits on `agent/<id>`), its own branch until then.
    pub fn branch(&self) -> String {
        match &self.state {
            crate::domain::task::TaskState::Done { branch, .. } => branch.clone(),
            _ => self.id.branch(),
        }
    }

    pub fn holds_vm(&self) -> bool {
        matches!(self.state, TaskState::Preparing | TaskState::Booting | TaskState::Running | TaskState::Collecting)
    }

    /// Adds a telemetry sample, keeping the last `HISTORY` CPU and memory percentages.
    pub(super) fn push_metrics(&mut self, m: VmMetrics) {
        // The same sample read again: the guest stopped writing, nothing new to count.
        if self.metrics.as_ref().is_some_and(|old| old.uptime_s == m.uptime_s) {
            return;
        }
        self.metrics_at = Some(now_s());
        for (history, value) in [(&mut self.cpu_history, m.cpu_pct as f32), (&mut self.mem_history, m.mem_pct() as f32)]
        {
            if history.len() == HISTORY {
                history.pop_front();
            }
            history.push_back(value);
        }
        self.metrics = Some(m);
    }
}

/// Samples of live telemetry kept: 5 minutes at one a second.
pub const LIVE: usize = 300;

impl TaskRecord {
    pub(super) fn push_live(&mut self, sample: TelemetrySample, memory_limit_mb: u64) {
        if self.live.len() == LIVE {
            self.live.pop_front();
        }
        self.live.push_back(sample);
        self.memory_limit_mb = Some(memory_limit_mb);
    }
}

fn now_s() -> f64 {
    SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}
