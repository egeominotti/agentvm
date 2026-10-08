//! A task's final outcome: a pure decision based on what the VM and guest left behind.

use std::time::Duration;

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
        // Whatever the guest saved before being stopped is still its work.
        return Outcome { final_: Final::Failed("timeout".into()), fetch: i.has_out_bundle };
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

/// Boot, clone, `.agentvm/setup.sh` (up to 30 minutes) and a Claude Code install: the longest a
/// guest may take before Claude starts.
pub const PREPARE_MAX: Duration = Duration::from_secs(2400);
/// After the guest's own limit: the 30 s it gives Claude to exit, then the final save.
pub const TIMEOUT_GRACE: Duration = Duration::from_secs(180);

/// Time after the VM's start when the server stops an automatic task the guest did not stop
/// itself. The guest's limit starts when Claude starts (`claude_started`, since the VM's start).
pub fn backstop(limit: Duration, claude_started: Option<Duration>) -> Duration {
    claude_started.unwrap_or(PREPARE_MAX) + limit + TIMEOUT_GRACE
}
