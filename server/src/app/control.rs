//! Structured control for any running VM, including headless agents.
use super::context::AppCtx;
use crate::adapters::{control, jobdir::JobWorkspace};
use crate::domain::{guest_control, ids::TaskId, task::TaskState};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("task not found")]
    NotFound,
    #[error("the VM is not running")]
    NotRunning,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Unreachable(String),
}

pub async fn request(ctx: &AppCtx, id: &TaskId, op: &str, args: Value) -> Result<Value, ControlError> {
    guest_control::validate(op, &args).map_err(ControlError::Invalid)?;
    let record = ctx.store.get(id).ok_or(ControlError::NotFound)?;
    if record.state != TaskState::Running {
        return Err(ControlError::NotRunning);
    }
    let socket = JobWorkspace::pty_socket_of(&ctx.config.jobs(), id);
    control::request(&socket, op, args).await.map_err(ControlError::Unreachable)
}
