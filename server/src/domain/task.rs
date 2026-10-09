//! Task lifecycle: states, events and the single function that connects them.

use serde::{Deserialize, Serialize};

use super::outcome::Final;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum TaskState {
    Queued,
    Preparing,
    Booting,
    Running,
    Collecting,
    Done { branch: String, commits: u32 },
    NoChanges,
    Failed { reason: String },
    Stopped,
}

/// When a task entered a state (`kind` of the state, seconds since the epoch).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct StateAt {
    pub state: String,
    pub at: f64,
}

impl StateAt {
    pub fn now(state: &TaskState) -> Self {
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
        StateAt { state: state.kind().to_owned(), at }
    }
}

impl TaskState {
    /// The state's name, without its details: `queued`, `running`, `failed`…
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Preparing => "preparing",
            Self::Booting => "booting",
            Self::Running => "running",
            Self::Collecting => "collecting",
            Self::Done { .. } => "done",
            Self::NoChanges => "no_changes",
            Self::Failed { .. } => "failed",
            Self::Stopped => "stopped",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done { .. } | Self::NoChanges | Self::Failed { .. } | Self::Stopped)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskEvent {
    SlotAcquired,
    Prepared,
    VmStarted,
    VmExited,
    /// Decided outcome + the task's branch name.
    Finished(Final, String),
    Failure(String),
    StopRequested,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid transition: {from:?} + {event:?}")]
pub struct InvalidTransition {
    pub from: TaskState,
    pub event: TaskEvent,
}

pub fn transition(from: &TaskState, event: &TaskEvent) -> Result<TaskState, InvalidTransition> {
    use TaskEvent as E;
    use TaskState as S;
    let next = match (from, event) {
        (s, _) if s.is_terminal() => None,
        (S::Queued, E::SlotAcquired) => Some(S::Preparing),
        (S::Preparing, E::Prepared) => Some(S::Booting),
        (S::Booting, E::VmStarted) => Some(S::Running),
        (S::Booting | S::Running, E::VmExited) => Some(S::Collecting),
        (S::Collecting, E::Finished(f, branch)) => Some(match f {
            Final::Done { commits } => S::Done { branch: branch.clone(), commits: *commits },
            Final::NoChanges => S::NoChanges,
            Final::Failed(reason) => S::Failed { reason: reason.clone() },
            Final::Stopped => S::Stopped,
        }),
        (_, E::Failure(reason)) => Some(S::Failed { reason: reason.clone() }),
        (S::Queued, E::StopRequested) => Some(S::Stopped),
        (s, E::StopRequested) => Some(s.clone()),
        _ => None,
    };
    next.ok_or_else(|| InvalidTransition { from: from.clone(), event: event.clone() })
}
