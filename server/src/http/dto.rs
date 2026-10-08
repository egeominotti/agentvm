//! Forme JSON esposte dall'API.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::app::store::TaskRecord;
use crate::domain::task::TaskState;

#[derive(Deserialize)]
pub struct CreateTask {
    pub repo_path: String,
    pub prompt: String,
    #[serde(default)]
    pub base_ref: Option<String>,
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
}

#[derive(Serialize)]
pub struct TaskDto {
    pub id: String,
    pub repo: String,
    pub prompt: String,
    pub base_sha: String,
    pub branch: String,
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
            prompt: r.prompt.as_str().to_owned(),
            base_sha: r.base_sha.as_str().to_owned(),
            status: r.state,
            created_at: unix(r.created_at),
            finished_at: r.finished_at.map(unix),
        }
    }
}

fn unix(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// Errore con messaggio per l'utente: `{"error": "..."}`.
pub struct ApiError(pub StatusCode, pub String);

impl ApiError {
    pub fn not_found() -> Self {
        ApiError(StatusCode::NOT_FOUND, "task inesistente".into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}
