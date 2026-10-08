use std::time::{Duration, UNIX_EPOCH};

use agentvm::domain::agent_event::{AgentEvent, parse_line};
use agentvm::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};
use agentvm::domain::outcome::{Final, GuestResult, GuestStatus, OutcomeInput, VmExit, decide};
use agentvm::domain::spec::TaskSpec;
use agentvm::domain::task::{TaskEvent, TaskState, transition};

// ---- ids ----

#[test]
fn task_id_has_timestamp_and_random_suffix() {
    // 2026-10-08 15:45:01 UTC
    let now = UNIX_EPOCH + Duration::from_secs(1_791_474_301);
    let id = TaskId::generate(now, [0xa3, 0xf9]);
    assert_eq!(id.as_str(), "20261008-154501-a3f9");
    assert_eq!(id.branch(), "agent/20261008-154501-a3f9");
}

#[test]
fn commit_sha_requires_40_hex() {
    assert!(CommitSha::parse(&"a".repeat(40)).is_ok());
    assert!(CommitSha::parse("abc").is_err());
    assert!(CommitSha::parse(&"g".repeat(40)).is_err());
}

#[test]
fn prompt_rejects_blank() {
    assert!(Prompt::new("   \n".into()).is_err());
    assert_eq!(Prompt::new("  fix it ".into()).unwrap().as_str(), "fix it");
}

#[test]
fn repo_path_requires_git_dir() {
    let dir = tempfile::tempdir().unwrap();
    assert!(RepoPath::new(dir.path().to_path_buf()).is_err());
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    assert!(RepoPath::new(dir.path().to_path_buf()).is_ok());
}

// ---- spec ----

#[test]
fn task_spec_roundtrips_hostile_prompt() {
    let spec = TaskSpec {
        id: "20261008-154501-a3f9".into(),
        prompt: "it's $(rm -rf /)\nok \"quoted\"".into(),
        branch: "agent/20261008-154501-a3f9".into(),
        base_sha: "a".repeat(40),
        timeout_s: 1800,
        interactive: false,
    };
    let json = serde_json::to_string(&spec).unwrap();
    let back: TaskSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(back, spec);
}

// ---- task state machine ----

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

// ---- outcome ----

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
    let r: GuestResult =
        serde_json::from_str(r#"{"status":"no_changes","claude_exit":0,"commits":0}"#).unwrap();
    assert_eq!(r.status, GuestStatus::NoChanges);
    assert_eq!(r.error, None);
}

// ---- agent events (real recorded stream) ----

#[test]
fn parses_real_stream() {
    let events: Vec<AgentEvent> = include_str!("fixtures/stream-hello.jsonl").lines().flat_map(parse_line).collect();
    assert!(matches!(&events[0], AgentEvent::Init { model } if model == "claude-sonnet-5-5"));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::ToolUse { name, summary } if name == "Bash" && summary.contains("hello.txt"))));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::ToolResult { is_error: false, summary } if summary.contains("aarch64"))));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Text { text } if text.contains("hello.txt"))));
    assert!(matches!(events.last().unwrap(), AgentEvent::Result { is_error: false, duration_ms: 6534, .. }));
    assert!(!events.iter().any(|e| matches!(e, AgentEvent::Unparsed { .. })));
}

#[test]
fn truncated_line_is_unparsed() {
    let ev = parse_line("{\"type\":\"assi");
    assert!(matches!(ev.as_slice(), [AgentEvent::Unparsed { raw }] if raw == "{\"type\":\"assi"));
}

#[test]
fn retry_event() {
    let ev = parse_line(r#"{"type":"system","subtype":"api_retry","attempt":3,"max_retries":10}"#);
    assert!(matches!(ev.as_slice(), [AgentEvent::Retry { attempt: 3 }]));
}

#[test]
fn long_summaries_are_truncated() {
    let long = "x".repeat(1000);
    let line = serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command": long}}]}}).to_string();
    match parse_line(&line).as_slice() {
        [AgentEvent::ToolUse { summary, .. }] => assert!(summary.chars().count() <= 301),
        other => panic!("{other:?}"),
    }
}

#[test]
fn agent_event_serializes_with_kind() {
    let v = serde_json::to_value(AgentEvent::Retry { attempt: 1 }).unwrap();
    assert_eq!(v, serde_json::json!({"kind": "retry", "attempt": 1}));
}

#[test]
fn task_spec_interactive_defaults_to_false() {
    let spec: TaskSpec = serde_json::from_str(
        r#"{"id":"x","prompt":"p","branch":"agent/x","base_sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","timeout_s":1}"#,
    )
    .unwrap();
    assert!(!spec.interactive);
}
