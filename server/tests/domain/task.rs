//! The task state machine.

use agentvm::domain::outcome::Final;
use agentvm::domain::task::{TaskEvent, TaskState, transition};

fn ok(s: TaskState, e: TaskEvent) -> TaskState {
    transition(&s, &e).unwrap_or_else(|err| panic!("{s:?} + {e:?}: {err}"))
}

#[test]
fn happy_path_transitions() {
    let s = ok(TaskState::Queued, TaskEvent::SlotAcquired);
    assert_eq!(s, TaskState::Preparing);
    let s = ok(s, TaskEvent::Prepared);
    assert_eq!(s, TaskState::Booting);
    let s = ok(s, TaskEvent::VmStarted);
    assert_eq!(s, TaskState::Running);
    let s = ok(s, TaskEvent::VmExited);
    assert_eq!(s, TaskState::Collecting);
    let s = ok(s, TaskEvent::Finished(Final::Done { commits: 2 }, "agent/x".into()));
    assert_eq!(s, TaskState::Done { branch: "agent/x".into(), commits: 2 });
    assert!(s.is_terminal());
}

#[test]
fn vm_can_exit_while_booting() {
    assert_eq!(ok(TaskState::Booting, TaskEvent::VmExited), TaskState::Collecting);
}

#[test]
fn stop_while_queued_stops_immediately() {
    assert_eq!(ok(TaskState::Queued, TaskEvent::StopRequested), TaskState::Stopped);
}

#[test]
fn stop_while_running_keeps_state_until_finished() {
    assert_eq!(ok(TaskState::Running, TaskEvent::StopRequested), TaskState::Running);
    let s = ok(TaskState::Collecting, TaskEvent::Finished(Final::Stopped, "agent/x".into()));
    assert_eq!(s, TaskState::Stopped);
}

#[test]
fn failure_from_any_active_state() {
    for s in [TaskState::Queued, TaskState::Preparing, TaskState::Booting, TaskState::Running, TaskState::Collecting] {
        assert_eq!(ok(s, TaskEvent::Failure("boom".into())), TaskState::Failed { reason: "boom".into() });
    }
}

#[test]
fn terminal_states_reject_events() {
    let done = TaskState::Done { branch: "b".into(), commits: 1 };
    assert!(transition(&done, &TaskEvent::VmStarted).is_err());
    assert!(transition(&TaskState::Stopped, &TaskEvent::StopRequested).is_err());
    assert!(transition(&TaskState::Queued, &TaskEvent::VmStarted).is_err());
}

#[test]
fn finished_maps_each_final() {
    let c = || TaskState::Collecting;
    assert_eq!(ok(c(), TaskEvent::Finished(Final::NoChanges, "b".into())), TaskState::NoChanges);
    assert_eq!(
        ok(c(), TaskEvent::Finished(Final::Failed("timeout".into()), "b".into())),
        TaskState::Failed { reason: "timeout".into() }
    );
}

#[test]
fn state_serializes_with_tag() {
    let v = serde_json::to_value(TaskState::Failed { reason: "x".into() }).unwrap();
    assert_eq!(v, serde_json::json!({"state": "failed", "reason": "x"}));
}
