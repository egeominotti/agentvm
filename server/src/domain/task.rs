//! Ciclo di vita di un task: stati, eventi e l'unica funzione che li collega.

use serde::Serialize;

use super::outcome::Final;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
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

impl TaskState {
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
    /// Esito deciso + nome del branch del task.
    Finished(Final, String),
    Failure(String),
    StopRequested,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("transizione non valida: {from:?} + {event:?}")]
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
