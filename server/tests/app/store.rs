//! The task store: transitions, stops, metrics and persistence across restarts.

use std::time::SystemTime;

use agentvm::app::events::StreamItem;
use agentvm::app::store::{Store, StoreError};
use agentvm::domain::ids::TaskId;
use agentvm::domain::task::{TaskEvent, TaskState};

use crate::helpers::record;

#[test]
fn store_applies_valid_transitions_and_logs_states() {
    let repo = tempfile::tempdir().unwrap();
    let store = Store::new();
    let rec = record(&repo);
    let id = rec.id.clone();
    store.insert(rec);
    assert_eq!(store.apply(&id, TaskEvent::SlotAcquired).unwrap(), TaskState::Preparing);
    let (history, _) = store.log(&id).unwrap().subscribe();
    let states: Vec<_> = history
        .into_iter()
        .filter_map(|(_, i)| match i {
            StreamItem::State(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(states, [TaskState::Queued, TaskState::Preparing]);
}

#[test]
fn store_rejects_out_of_order_events() {
    let repo = tempfile::tempdir().unwrap();
    let store = Store::new();
    let rec = record(&repo);
    let id = rec.id.clone();
    store.insert(rec);
    assert!(matches!(store.apply(&id, TaskEvent::VmStarted), Err(StoreError::Invalid(_))));
    assert_eq!(store.get(&id).unwrap().state, TaskState::Queued);
}

#[test]
fn stopping_a_queued_task_stops_it_and_signals() {
    let repo = tempfile::tempdir().unwrap();
    let store = Store::new();
    let rec = record(&repo);
    let id = rec.id.clone();
    store.insert(rec);
    let signal = store.stop_signal(&id).unwrap();
    assert_eq!(store.request_stop(&id).unwrap(), TaskState::Stopped);
    assert!(*signal.borrow());
    let rec = store.get(&id).unwrap();
    assert!(rec.finished_at.is_some());
}

#[test]
fn unknown_task_is_not_found() {
    let store = Store::new();
    let id = TaskId::generate(SystemTime::now(), &[0, 0]);
    assert!(matches!(store.apply(&id, TaskEvent::SlotAcquired), Err(StoreError::NotFound)));
    assert!(store.get(&id).is_none());
}

#[test]
fn store_keeps_the_last_60_metric_samples() {
    use agentvm::domain::metrics::VmMetrics;
    let repo = tempfile::tempdir().unwrap();
    let store = Store::new();
    let rec = record(&repo);
    let id = rec.id.clone();
    store.insert(rec);
    for i in 0..70 {
        let m: VmMetrics = serde_json::from_value(serde_json::json!({
            "uptime_s": i, "cpus": 4, "cpu_pct": i as f64, "load1": 0.0, "mem_used_mb": 100, "mem_total_mb": 4000,
            "disk_used_mb": 1, "disk_total_mb": 2, "net_rx_bps": 0, "net_tx_bps": 0, "procs": 1, "top": []
        }))
        .unwrap();
        store.record_metrics(&id, m);
    }
    let rec = store.get(&id).unwrap();
    assert_eq!(rec.cpu_history.len(), 60);
    assert_eq!(rec.cpu_history.back().copied(), Some(69.0));
    assert_eq!(rec.metrics.unwrap().uptime_s, 69);
}

#[test]
fn a_persistent_store_reloads_tasks_after_a_restart() {
    let repo = tempfile::tempdir().unwrap();
    let jobs = tempfile::tempdir().unwrap();
    let rec = record(&repo);
    let id = rec.id.clone();
    {
        let store = Store::persistent(jobs.path().to_path_buf());
        store.insert(rec);
        store.apply(&id, TaskEvent::SlotAcquired).unwrap();
        store.apply(&id, TaskEvent::Prepared).unwrap();
    }
    let reloaded = Store::load(jobs.path());
    assert_eq!(reloaded.len(), 1);
    assert_eq!(reloaded[0].id, id);
    assert_eq!(reloaded[0].state, TaskState::Booting);
    assert_eq!(reloaded[0].prompt.as_ref().unwrap().as_str(), "do something");
}

#[test]
fn a_machine_keeps_its_own_snapshot_interval_across_restarts() {
    let repo = tempfile::tempdir().unwrap();
    let jobs = tempfile::tempdir().unwrap();
    let rec = record(&repo);
    let id = rec.id.clone();
    {
        let store = Store::persistent(jobs.path().to_path_buf());
        store.insert(rec);
        assert_eq!(store.get(&id).unwrap().auto_snapshot_min, None);
        store.set_auto_snapshot_min(&id, Some(5));
    }
    assert_eq!(Store::load(jobs.path())[0].auto_snapshot_min, Some(5));
}

/// A record written by an older or newer agentvm must not make the server kill a running VM.
#[test]
fn records_from_other_versions_load_and_unreadable_ones_are_reported() {
    let jobs = tempfile::tempdir().unwrap();
    let old = jobs.path().join("20261008-120000-aaaa");
    std::fs::create_dir_all(&old).unwrap();
    // No cpus, memory_mb, model, label, usage: written before those fields existed.
    std::fs::write(
        old.join("record.json"),
        format!(
            r#"{{"id":"20261008-120000-aaaa","repo":"{}","prompt":null,
        "base_sha":"{}","interactive":true,"state":{{"state":"running"}},"claude_version":null,"restore_from":null,
        "created_at":{{"secs_since_epoch":1,"nanos_since_epoch":0}},"finished_at":null}}"#,
            jobs.path().display(),
            "a".repeat(40)
        ),
    )
    .unwrap();
    let broken = jobs.path().join("20261008-120000-bbbb");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("record.json"), "{ half a reco").unwrap();

    let (records, unreadable) = Store::load_all(jobs.path());
    assert_eq!(records.len(), 1, "the old record did not load");
    assert_eq!(records[0].state, TaskState::Running);
    assert_eq!(unreadable, vec!["20261008-120000-bbbb".to_owned()]);
}

/// Two tasks launched in the same second with the same random bits: the second is refused
/// instead of silently replacing the first one (whose VM would never be supervised again).
#[test]
fn a_task_id_already_in_use_is_refused() {
    let repo = tempfile::tempdir().unwrap();
    let store = Store::new();
    let first = record(&repo);
    let mut second = first.clone();
    second.label = Some("second".into());
    store.try_insert(first.clone()).unwrap();
    assert!(store.try_insert(second).is_err());
    assert_eq!(store.get(&first.id).unwrap().label, None);
}

/// Work that landed on `agent/<id>-vm` (the user had commits on `agent/<id>`) is shown, diffed
/// and merged from there, never from the branch it did not reach.
#[test]
fn a_finished_task_points_at_the_branch_its_work_landed_on() {
    use agentvm::domain::outcome::Final;
    let repo = tempfile::tempdir().unwrap();
    let store = Store::new();
    let rec = record(&repo);
    let id = rec.id.clone();
    store.insert(rec);
    assert_eq!(store.get(&id).unwrap().branch(), id.branch(), "before it ends: its own branch");
    for e in [TaskEvent::SlotAcquired, TaskEvent::Prepared, TaskEvent::VmStarted, TaskEvent::VmExited] {
        store.apply(&id, e).unwrap();
    }
    let landed = format!("{}-vm", id.branch());
    store.apply(&id, TaskEvent::Finished(Final::Done { commits: 1 }, landed.clone())).unwrap();
    assert_eq!(store.get(&id).unwrap().branch(), landed);
}

/// Each state a task goes through is kept with its time, across a restart, for its diagnostics.
#[test]
fn a_task_keeps_the_timeline_of_its_states() {
    let repo = tempfile::tempdir().unwrap();
    let jobs = tempfile::tempdir().unwrap();
    let rec = record(&repo);
    let id = rec.id.clone();
    let store = Store::persistent(jobs.path().to_path_buf());
    store.insert(rec);
    store.apply(&id, TaskEvent::SlotAcquired).unwrap();
    store.apply(&id, TaskEvent::Failure("boom".into())).unwrap();
    let timeline = Store::load(jobs.path()).remove(0).timeline;
    let states: Vec<_> = timeline.iter().map(|s| s.state.as_str()).collect();
    assert_eq!(states, ["queued", "preparing", "failed"]);
    assert!(timeline.windows(2).all(|w| w[0].at <= w[1].at) && timeline[0].at > 1.7e9, "{timeline:?}");
}

/// Records written before the timeline existed still load (with an empty one).
#[test]
fn a_record_without_a_timeline_still_loads() {
    let repo = tempfile::tempdir().unwrap();
    let mut json = serde_json::to_value(record(&repo)).unwrap();
    json.as_object_mut().unwrap().remove("timeline");
    let back: agentvm::app::store::TaskRecord = serde_json::from_value(json).unwrap();
    assert!(back.timeline.is_empty());
}
