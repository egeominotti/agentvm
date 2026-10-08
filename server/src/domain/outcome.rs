//! A task's final outcome: a pure decision based on what the VM and guest left behind.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestStatus {
    Ok,
    NoChanges,
    Failed,
}

/// `result.json` written by `agentvm-job` in the guest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GuestResult {
    pub status: GuestStatus,
    pub claude_exit: i32,
    pub commits: u32,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmExit {
    /// The guest shut down on its own.
    Clean,
    /// The VM process reported an error or exited unexpectedly.
    Error(String),
    /// Stopped by us (SIGTERM).
    Signaled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeInput {
    pub exit: VmExit,
    pub result: Option<GuestResult>,
    pub stop_requested: bool,
    pub timed_out: bool,
    pub has_out_bundle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Final {
    Done { commits: u32 },
    NoChanges,
    Failed(String),
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub final_: Final,
    /// Import `out.bundle` into the local repo.
    pub fetch: bool,
}

pub fn decide(i: &OutcomeInput) -> Outcome {
    let no_fetch = |final_| Outcome { final_, fetch: false };
    if i.stop_requested {
        return no_fetch(Final::Stopped);
    }
    if i.timed_out {
        return no_fetch(Final::Failed("timeout".into()));
    }
    if let VmExit::Error(m) = &i.exit {
        return no_fetch(Final::Failed(format!("vm_error: {m}")));
    }
    let Some(r) = &i.result else {
        return no_fetch(Final::Failed("guest_no_result".into()));
    };
    match r.status {
        GuestStatus::Ok if i.has_out_bundle => Outcome { final_: Final::Done { commits: r.commits }, fetch: true },
        GuestStatus::Ok => no_fetch(Final::Failed("missing_out_bundle".into())),
        GuestStatus::NoChanges => no_fetch(Final::NoChanges),
        GuestStatus::Failed => Outcome {
            final_: Final::Failed(r.error.clone().unwrap_or_else(|| format!("claude_exit {}", r.claude_exit))),
            fetch: i.has_out_bundle,
        },
    }
}
