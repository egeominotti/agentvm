//! Automatic snapshots: when they are due and which ones are pruned.

use std::time::Duration;

use agentvm::domain::snapshot::{AutoSnapshots, SnapshotId, SnapshotMeta};

fn snap(task: &str, at: f64, auto: bool) -> SnapshotMeta {
    SnapshotMeta {
        id: SnapshotId::generate(std::time::UNIX_EPOCH + Duration::from_secs_f64(at), &[at as u8, auto as u8]),
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

/// A machine started from a snapshot says where it came from, once: a kept disk is resumed.
#[test]
fn a_restored_machine_is_named_after_its_snapshot() {
    use agentvm::domain::snapshot::{restored_label, title_of_label};
    assert_eq!(restored_label("Fix the cart"), "Restored: Fix the cart");
    assert_eq!(restored_label("Interrupted: setup-fail, 03:21"), "Resumed: setup-fail, 03:21");
    assert_eq!(title_of_label("Restored: Fix the cart"), "Fix the cart");
    assert_eq!(title_of_label("Resumed: setup-fail, 03:21"), "setup-fail, 03:21");
    assert_eq!(title_of_label("Spike"), "Spike");
}

/// The clock set back (a time zone fix, a wrong NTP answer) must not pause automatic snapshots
/// until it catches up with the last one.
#[test]
fn a_clock_set_back_does_not_pause_automatic_snapshots() {
    let every_30 = AutoSnapshots { every_min: 30, ..Default::default() };
    let last = 10_000.0;
    assert!(every_30.due(Some(last), 0.0, last - 3600.0), "an hour back: due at once");
    assert!(!every_30.due(Some(last), 0.0, last - 30.0), "a few seconds of drift is not a jump");
}
