//! JSON shapes exposed by the API.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
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
}

#[derive(Serialize)]
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
pub struct Saved {
    pub commits: u32,
}

#[derive(Deserialize)]
pub struct PtyQuery {
    #[serde(default = "default_session")]
    pub session: String,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
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
pub struct Created {
    pub id: String,
}

#[derive(Serialize)]
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
    pub usage: Option<crate::domain::usage::AgentUsage>,
    pub metrics: Option<VmMetrics>,
    pub cpu_history: Vec<f32>,
    pub mem_history: Vec<f32>,
    pub status: TaskState,
    pub created_at: f64,
    pub finished_at: Option<f64>,
}

impl From<TaskRecord> for TaskDto {
    fn from(r: TaskRecord) -> Self {
        TaskDto {
            branch: r.id.branch(),
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
            usage: r.usage,
            metrics: r.metrics,
            cpu_history: r.cpu_history.into(),
            mem_history: r.mem_history.into(),
            status: r.state,
            created_at: unix(r.created_at),
            finished_at: r.finished_at.map(unix),
        }
    }
}

fn unix(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// Error with a user-facing message: `{"error": "..."}`.
pub struct ApiError(pub StatusCode, pub String);

impl ApiError {
    pub fn not_found() -> Self {
        ApiError(StatusCode::NOT_FOUND, "task not found".into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}
