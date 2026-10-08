//! Letture che coinvolgono sistemi esterni, esposte all'HTTP senza fargli conoscere gli adapter.

use super::supervisor::AppCtx;
use crate::adapters::git::Git;
use crate::domain::ids::TaskId;
use crate::domain::task::TaskState;

#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("task inesistente")]
    NotFound,
    #[error("il task non ha prodotto un branch")]
    NoBranch,
    #[error("{0}")]
    Git(String),
}

/// Diff tra il commit di partenza e il branch prodotto dal task.
pub async fn task_diff(ctx: &AppCtx, id: &TaskId) -> Result<String, DiffError> {
    let record = ctx.store.get(id).ok_or(DiffError::NotFound)?;
    if !matches!(record.state, TaskState::Done { .. }) {
        return Err(DiffError::NoBranch);
    }
    let branch = id.branch();
    tokio::task::spawn_blocking(move || Git::new(record.repo).diff(&record.base_sha, &branch))
        .await
        .map_err(|e| DiffError::Git(e.to_string()))?
        .map_err(|e| DiffError::Git(e.to_string()))
}
