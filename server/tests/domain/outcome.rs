//! How a VM's exit and the guest's result decide a task's outcome.

use std::time::Duration;

use agentvm::domain::outcome::{Final, GuestResult, GuestStatus, OutcomeInput, VmExit, decide};

fn guest(status: GuestStatus, exit: i32, commits: u32, error: Option<&str>) -> Option<GuestResult> {
    Some(GuestResult { status, claude_exit: exit, commits, error: error.map(Into::into) })
}

fn input(exit: VmExit, result: Option<GuestResult>, bundle: bool) -> OutcomeInput {
    OutcomeInput { exit, result, stop_requested: false, timed_out: false, has_out_bundle: bundle }
}

#[test]
fn stop_wins_over_everything() {
    let mut i = input(VmExit::Signaled, guest(GuestStatus::Ok, 0, 1, None), true);
    i.stop_requested = true;
    i.timed_out = true;
    let o = decide(&i);
    assert_eq!(o.final_, Final::Stopped);
    assert!(!o.fetch);
}

#[test]
fn timeout_fails_without_fetch() {
    let mut i = input(VmExit::Signaled, None, false);
    i.timed_out = true;
    assert_eq!(decide(&i).final_, Final::Failed("timeout".into()));
    assert!(!decide(&i).fetch);
}

/// The server stopped a VM past its limit, but the guest had saved its work: import it.
#[test]
fn a_timeout_still_imports_what_the_guest_saved() {
    let mut i = input(VmExit::Signaled, None, true);
    i.timed_out = true;
    let o = decide(&i);
    assert_eq!(o.final_, Final::Failed("timeout".into()));
    assert!(o.fetch);
}

/// The guest's limit starts when Claude starts; boot and a 30-minute setup.sh come before it.
#[test]
fn the_server_backstop_counts_from_when_claude_starts() {
    use agentvm::domain::outcome::{PREPARE_MAX, TIMEOUT_GRACE, backstop};
    let limit = Duration::from_secs(600);
    let started = Duration::from_secs(1500);
    assert_eq!(backstop(limit, Some(started)), started + limit + TIMEOUT_GRACE);
    // Not started (yet, or the marker was lost): never before the longest preparation.
    assert_eq!(backstop(limit, None), PREPARE_MAX + limit + TIMEOUT_GRACE);
    assert!(PREPARE_MAX >= Duration::from_secs(1800 + 300), "setup.sh may take 30 minutes");
}

#[test]
fn vm_error_is_reported() {
    let o = decide(&input(VmExit::Error("no disk".into()), None, false));
    assert_eq!(o.final_, Final::Failed("vm_error: no disk".into()));
}

#[test]
fn missing_result_is_guest_no_result() {
    assert_eq!(decide(&input(VmExit::Clean, None, false)).final_, Final::Failed("guest_no_result".into()));
}

#[test]
fn ok_with_bundle_is_done_and_fetched() {
    let o = decide(&input(VmExit::Clean, guest(GuestStatus::Ok, 0, 3, None), true));
    assert_eq!(o.final_, Final::Done { commits: 3 });
    assert!(o.fetch);
}

#[test]
fn ok_without_bundle_fails() {
    let o = decide(&input(VmExit::Clean, guest(GuestStatus::Ok, 0, 3, None), false));
    assert_eq!(o.final_, Final::Failed("missing_out_bundle".into()));
}

#[test]
fn no_changes() {
    let o = decide(&input(VmExit::Clean, guest(GuestStatus::NoChanges, 0, 0, None), false));
    assert_eq!(o.final_, Final::NoChanges);
    assert!(!o.fetch);
}

#[test]
fn guest_failed_still_fetches_commits() {
    let o = decide(&input(VmExit::Clean, guest(GuestStatus::Failed, 1, 1, None), true));
    assert_eq!(o.final_, Final::Failed("claude_exit 1".into()));
    assert!(o.fetch);
    let o = decide(&input(VmExit::Clean, guest(GuestStatus::Failed, 0, 0, Some("no_task")), false));
    assert_eq!(o.final_, Final::Failed("no_task".into()));
    assert!(!o.fetch);
}

#[test]
fn guest_result_parses_guest_json() {
    let r: GuestResult = serde_json::from_str(r#"{"status":"no_changes","claude_exit":0,"commits":0}"#).unwrap();
    assert_eq!(r.status, GuestStatus::NoChanges);
    assert_eq!(r.error, None);
}
