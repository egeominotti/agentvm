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
        model: None,
        claude_version: None,
        restore: false,
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

// ---- agent events (real recorded stream) ----

#[test]
fn parses_real_stream() {
    let events: Vec<AgentEvent> = include_str!("fixtures/stream-hello.jsonl").lines().flat_map(parse_line).collect();
    assert!(
        matches!(&events[0], AgentEvent::Init { model, claude_code_version } if model == "claude-sonnet-5-5" && claude_code_version == "2.1.294")
    );
    assert!(events.iter().any(
        |e| matches!(e, AgentEvent::ToolUse { name, summary } if name == "Bash" && summary.contains("hello.txt"))
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolResult { is_error: false, summary } if summary.contains("aarch64")))
    );
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

// ---- settings ----

use agentvm::domain::settings::{HostLimits, Model, Settings};

fn limits() -> HostLimits {
    HostLimits { cpus: 18, ram_mb: 65536 }
}

#[test]
fn default_settings_are_valid() {
    let s = Settings {
        max_vms: 4,
        cpus: 4,
        memory_mb: 4096,
        timeout_s: 1800,
        model: Model::default_choice(),
        default_repo: None,
        claude_version: Default::default(),
        s3: None,
        auto_snapshots: Default::default(),
    };
    assert!(s.validate(&limits()).is_ok());
}

#[test]
fn settings_reject_out_of_range_values() {
    let ok = Settings {
        max_vms: 4,
        cpus: 4,
        memory_mb: 4096,
        timeout_s: 1800,
        model: Model::default_choice(),
        default_repo: None,
        claude_version: Default::default(),
        s3: None,
        auto_snapshots: Default::default(),
    };
    for bad in [
        Settings { max_vms: 0, ..ok.clone() },
        Settings { cpus: 0, ..ok.clone() },
        Settings { cpus: 19, ..ok.clone() },
        Settings { memory_mb: 512, ..ok.clone() },
        Settings { memory_mb: 65536, ..ok.clone() },
        Settings { timeout_s: 10, ..ok.clone() },
    ] {
        assert!(bad.validate(&limits()).is_err(), "{bad:?}");
    }
}

#[test]
fn recommended_vms_fit_in_ram() {
    assert_eq!(limits().recommended_vms(4096), 14);
    assert_eq!(limits().recommended_vms(2048), 28);
}

#[test]
fn model_maps_to_cli_flag() {
    assert_eq!(Model::default_choice().cli_name(), None);
    for ok in ["opus", "sonnet[1m]", "opusplan", "fable", "claude-opus-5-5", "claude-haiku-5-5"] {
        assert_eq!(Model::parse(ok).unwrap().cli_name(), Some(ok));
    }
    for bad in ["", "Opus", "opus; rm -rf /", "--dangerous", "a b"] {
        assert!(Model::parse(bad).is_err(), "{bad}");
    }
    let m: Model = serde_json::from_str("\"haiku\"").unwrap();
    assert_eq!(m.as_str(), "haiku");
}

#[test]
fn vm_metrics_parse_guest_json() {
    use agentvm::domain::metrics::VmMetrics;
    let m: VmMetrics = serde_json::from_str(
        r#"{"uptime_s":12,"cpus":4,"cpu_pct":37.5,"load1":0.8,"mem_used_mb":900,"mem_total_mb":3922,
            "disk_used_mb":3100,"disk_total_mb":19800,"net_rx_bps":1200,"net_tx_bps":300,"procs":91,
            "top":[{"name":"cargo","cpu_pct":180.2,"mem_mb":410}]}"#,
    )
    .unwrap();
    assert_eq!(m.top[0].name, "cargo");
    assert_eq!(m.cpus, 4);
}

#[test]
fn claude_version_accepts_channels_and_semver_only() {
    use agentvm::domain::settings::ClaudeVersion;
    for ok in ["latest", "stable", "2.1.294", "2.2.0-beta.1"] {
        assert_eq!(ClaudeVersion::parse(ok).unwrap().as_str(), ok);
    }
    for bad in ["", "2.1", "v2.1.0", "latest; rm -rf /", "2.1.0 --evil", "../../x"] {
        assert!(ClaudeVersion::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn settings_without_claude_version_default_to_latest() {
    let s: Settings = serde_json::from_str(
        r#"{"max_vms":4,"cpus":4,"memory_mb":4096,"timeout_s":1800,"model":"default","default_repo":null}"#,
    )
    .unwrap();
    assert_eq!(s.claude_version.as_str(), "latest");
}

// ---- S3 ----

use agentvm::domain::s3::S3Config;

fn s3(endpoint: &str, path_style: bool) -> S3Config {
    S3Config {
        endpoint: endpoint.into(),
        region: "auto".into(),
        bucket: "backups".into(),
        prefix: "agentvm".into(),
        access_key: "AK".into(),
        path_style,
    }
}

#[test]
fn s3_urls_follow_the_addressing_style() {
    assert_eq!(
        s3("https://acc.r2.cloudflarestorage.com", true).object_url("agentvm/x.json"),
        "https://acc.r2.cloudflarestorage.com/backups/agentvm/x.json"
    );
    assert_eq!(
        s3("https://s3.eu-central-1.amazonaws.com/", false).object_url("agentvm/x.json"),
        "https://backups.s3.eu-central-1.amazonaws.com/agentvm/x.json"
    );
    assert_eq!(s3("http://127.0.0.1:9100", true).bucket_url(), "http://127.0.0.1:9100/backups");
    assert_eq!(s3("http://127.0.0.1:9100", true).key("snap-1.tar.zst"), "agentvm/snap-1.tar.zst");
}

#[test]
fn s3_config_is_validated() {
    assert!(s3("https://fsn1.your-objectstorage.com", false).validate().is_ok());
    assert!(s3("ftp://x", true).validate().is_err());
    assert!(S3Config { bucket: "Bad_Bucket".into(), ..s3("https://x.com", true) }.validate().is_err());
    assert!(S3Config { region: "".into(), ..s3("https://x.com", true) }.validate().is_err());
    assert!(S3Config { access_key: "a b".into(), ..s3("https://x.com", true) }.validate().is_err());
}

#[test]
fn per_vm_resources_are_checked_against_the_mac() {
    let host = limits();
    assert!(host.check_vm(4, 4096).is_ok());
    assert!(host.check_vm(18, 8192).is_ok());
    assert!(host.check_vm(0, 4096).is_err());
    assert!(host.check_vm(19, 4096).is_err());
    assert!(host.check_vm(4, 512).is_err());
    assert!(host.check_vm(4, 65536).is_err());
}

#[test]
fn agent_usage_parses_the_status_line_copy() {
    use agentvm::domain::usage::AgentUsage;
    let u: AgentUsage = serde_json::from_str(
        r#"{"cost_usd":0.1234,"input_tokens":15234,"output_tokens":2100,"lines_added":12,"lines_removed":3,"context_pct":7,"model":"Sonnet 5.5"}"#,
    )
    .unwrap();
    assert_eq!(u.input_tokens, 15234);
    assert!((u.cost_usd - 0.1234).abs() < 1e-9);
    let empty: AgentUsage = serde_json::from_str("{}").unwrap();
    assert_eq!(empty.output_tokens, 0);
}

use agentvm::domain::save::SaveReply;

#[test]
fn save_reply_waits_for_complete_json_and_reports_guest_errors() {
    // The guest may be halfway through writing the file when the host looks.
    assert_eq!(SaveReply::parse(b""), None);
    assert_eq!(SaveReply::parse(b"{\"comm"), None);
    assert_eq!(SaveReply::parse(br#"{"commits":2}"#), Some(SaveReply::Saved { commits: 2 }));
    assert_eq!(
        SaveReply::parse(br#"{"commits":0,"error":"git commit failed: index.lock exists"}"#),
        Some(SaveReply::Failed("git commit failed: index.lock exists".into()))
    );
}

use agentvm::domain::hostname;

#[test]
fn every_vm_gets_a_short_stable_dns_name() {
    let id = TaskId::parse("20261008-185855-4f94").unwrap();
    assert_eq!(hostname::vm_name("demo-web", &id), "demo-web-4f94");
    assert_eq!(hostname::vm_name("Fix the parser in src/lib.rs!", &id), "fix-the-parser-in-src-lib-4f94");
    assert_eq!(hostname::vm_name("   ", &id), "vm-4f94");
    assert_eq!(hostname::vm_name("Ünïcode ✨ app", &id), "n-code-app-4f94");
    assert!(hostname::vm_name(&"x".repeat(200), &id).len() <= 40);
}

#[test]
fn proxy_hosts_are_recognised_only_on_this_server() {
    assert_eq!(
        hostname::parse_proxy_host("3000.demo-web-4f94.localhost:7777", 7777),
        Some((3000, "demo-web-4f94".into()))
    );
    assert_eq!(hostname::parse_proxy_host("3000.demo-web-4f94.localhost:7778", 7777), None);
    assert_eq!(hostname::parse_proxy_host("127.0.0.1:7777", 7777), None);
    assert_eq!(hostname::parse_proxy_host("localhost:7777", 7777), None);
    assert_eq!(hostname::parse_proxy_host("demo-web-4f94.localhost:7777", 7777), None);
    assert_eq!(hostname::parse_proxy_host("99999.demo-4f94.localhost:7777", 7777), None);
    assert_eq!(hostname::parse_proxy_host("3000.a.b.localhost:7777", 7777), None);
    assert_eq!(hostname::proxy_url(3000, "demo-web-4f94", 7777), "http://3000.demo-web-4f94.localhost:7777");
}

use agentvm::domain::snapshot::{AutoSnapshots, SnapshotId, SnapshotMeta};

fn snap(task: &str, at: f64, auto: bool) -> SnapshotMeta {
    SnapshotMeta {
        id: SnapshotId::generate(std::time::UNIX_EPOCH + Duration::from_secs_f64(at), [at as u8, auto as u8]),
        name: String::new(),
        source_task: task.into(),
        repo: "/r".into(),
        base_sha: "a".repeat(40),
        model: "default".into(),
        claude_version: None,
        created_at: at,
        size_mb: 0,
        cpus: 0,
        memory_mb: 0,
        auto,
    }
}

#[test]
fn automatic_snapshots_are_due_every_interval_from_the_last_one() {
    let p = AutoSnapshots { every_min: 30, keep: 3, before_close: true };
    // Counted from the VM's start until the first automatic snapshot exists.
    assert!(!p.due(None, 1_000.0, 1_000.0 + 29.0 * 60.0));
    assert!(p.due(None, 1_000.0, 1_000.0 + 30.0 * 60.0));
    assert!(!p.due(Some(5_000.0), 1_000.0, 5_000.0 + 10.0 * 60.0));
    assert!(p.due(Some(5_000.0), 1_000.0, 5_000.0 + 31.0 * 60.0));
    let off = AutoSnapshots { every_min: 0, ..p };
    assert!(!off.due(None, 0.0, 1e9));
}

#[test]
fn pruning_keeps_the_newest_automatic_snapshots_and_never_manual_ones() {
    let p = AutoSnapshots { every_min: 30, keep: 2, before_close: true };
    let all = vec![
        snap("t1", 100.0, true),
        snap("t1", 200.0, false),
        snap("t1", 300.0, true),
        snap("t1", 400.0, true),
        snap("t2", 50.0, true),
    ];
    let gone = p.to_prune(&all, "t1");
    assert_eq!(gone, vec![all[0].id.clone()]);
    assert!(AutoSnapshots { keep: 5, ..p }.to_prune(&all, "t1").is_empty());
}

#[test]
fn automatic_snapshot_settings_are_validated() {
    assert!(AutoSnapshots { every_min: 30, keep: 4, before_close: true }.validate().is_ok());
    assert!(AutoSnapshots { every_min: 0, keep: 4, before_close: false }.validate().is_ok());
    assert!(AutoSnapshots { every_min: 7, keep: 4, before_close: true }.validate().is_err());
    assert!(AutoSnapshots { every_min: 30, keep: 0, before_close: true }.validate().is_err());
    assert!(AutoSnapshots { every_min: 30, keep: 51, before_close: true }.validate().is_err());
}

use agentvm::domain::memory::memory_target;

#[test]
fn idle_vms_give_memory_back_and_busy_ones_get_it_all() {
    // Busy, or not idle long enough: everything it was given.
    assert_eq!(memory_target(4096, 700, true, 600), 4096);
    assert_eq!(memory_target(4096, 700, false, 30), 4096);
    // Idle for a minute: what it uses plus 1 GB of headroom, in 256 MB steps, at least 1 GB.
    assert_eq!(memory_target(4096, 700, false, 61), 1792);
    assert_eq!(memory_target(4096, 100, false, 61), 1280);
    assert_eq!(memory_target(4096, 3500, false, 61), 4096);
    assert_eq!(memory_target(2048, 50, false, 3600), 1280);
}
