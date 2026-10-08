//! Store and events: in-memory logic, tested directly.

use std::time::{Duration, SystemTime};

use agentvm::app::events::{EventLog, StreamItem};
use agentvm::app::store::{Store, StoreError, TaskRecord};
use agentvm::domain::agent_event::AgentEvent;
use agentvm::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};
use agentvm::domain::task::{TaskEvent, TaskState};
use futures::StreamExt;

fn agent(n: u32) -> StreamItem {
    StreamItem::Agent(AgentEvent::Retry { attempt: n })
}

fn seqs(items: &[(u64, StreamItem)]) -> Vec<u64> {
    items.iter().map(|(s, _)| *s).collect()
}

#[tokio::test]
async fn late_subscriber_gets_history_then_live_without_duplicates() {
    let log = std::sync::Arc::new(EventLog::new());
    log.push(agent(1));
    log.push(agent(2));
    let early = log.stream();
    log.push(agent(3));
    let late = log.stream();
    log.push(agent(4));

    let take = |s| tokio::time::timeout(Duration::from_secs(2), StreamExt::take(s, 4).collect::<Vec<_>>());
    let early: Vec<(u64, StreamItem)> = take(early).await.unwrap();
    let late: Vec<(u64, StreamItem)> = take(late).await.unwrap();
    assert_eq!(seqs(&early), [1, 2, 3, 4]);
    assert_eq!(seqs(&late), [1, 2, 3, 4]);
}

fn record(repo: &tempfile::TempDir) -> TaskRecord {
    std::fs::create_dir_all(repo.path().join(".git")).unwrap();
    TaskRecord::new(
        TaskId::generate(SystemTime::now(), [9, 9]),
        RepoPath::new(repo.path().to_path_buf()).unwrap(),
        Some(Prompt::new("do something".into()).unwrap()),
        CommitSha::parse(&"b".repeat(40)).unwrap(),
        false,
    )
}

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
    let id = TaskId::generate(SystemTime::now(), [0, 0]);
    assert!(matches!(store.apply(&id, TaskEvent::SlotAcquired), Err(StoreError::NotFound)));
    assert!(store.get(&id).is_none());
}

#[tokio::test]
async fn scheduler_grows_and_shrinks_at_runtime() {
    use agentvm::app::scheduler::Scheduler;
    let s = Scheduler::new(1);
    let a = s.acquire().await;
    assert!(tokio::time::timeout(Duration::from_millis(50), s.acquire()).await.is_err(), "only 1 slot");
    s.resize(2);
    let b = tokio::time::timeout(Duration::from_millis(200), s.acquire()).await.expect("second slot after grow");
    assert_eq!(s.concurrency(), 2);
    s.resize(1);
    drop(a);
    drop(b);
    let _c = s.acquire().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(tokio::time::timeout(Duration::from_millis(100), s.acquire()).await.is_err(), "back to 1 slot");
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

fn ctx(home: &std::path::Path) -> std::sync::Arc<agentvm::app::supervisor::AppCtx> {
    let config = agentvm::config::Config {
        home: home.to_path_buf(),
        port: 7777,
        concurrency: 1,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 60,
        vm_helper: "agentvm-vm".into(),
        scripts_dir: "scripts".into(),
    };
    let keychain = agentvm::adapters::keychain::Keychain::new(Some(home.join("none.keychain-db")));
    std::sync::Arc::new(agentvm::app::supervisor::AppCtx::new(config, keychain))
}

/// A snapshot folder as `take_snapshot` leaves it, with a small disk.
fn stored_snapshot(home: &std::path::Path) -> agentvm::domain::snapshot::SnapshotId {
    use agentvm::domain::snapshot::{SnapshotId, SnapshotMeta};
    let id = SnapshotId::generate(SystemTime::now(), [1, 2]);
    let dir = home.join("snapshots").join(id.as_str());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("disk.raw"), vec![7u8; 4 << 20]).unwrap();
    std::fs::write(dir.join("efivars"), b"efi").unwrap();
    let meta = SnapshotMeta {
        id: id.clone(),
        name: "s".into(),
        source_task: "t".into(),
        repo: "/r".into(),
        base_sha: "a".repeat(40),
        model: "default".into(),
        claude_version: None,
        created_at: 1.0,
        size_mb: 4,
        cpus: 0,
        memory_mb: 0,
        auto: false,
    };
    std::fs::write(dir.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
    id
}

/// A download and an S3 backup of the same snapshot at once must not share a temporary file.
#[tokio::test]
async fn simultaneous_exports_of_a_snapshot_do_not_corrupt_each_other() {
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    let id = stored_snapshot(home.path());
    let (a, b) = tokio::join!(agentvm::app::backups::export(&ctx, &id), agentvm::app::backups::export(&ctx, &id));
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a, b, "both exports wrote the same file");
    for archive in [&a, &b] {
        let out = tempfile::tempdir().unwrap();
        agentvm::adapters::archive::unpack(archive, out.path()).unwrap();
        assert_eq!(std::fs::read(out.path().join("disk.raw")).unwrap().len(), 4 << 20);
    }
}

/// Imports at the same time each get their own scratch folder.
#[tokio::test]
async fn simultaneous_imports_each_produce_a_complete_snapshot() {
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    let id = stored_snapshot(home.path());
    let file = agentvm::app::backups::export(&ctx, &id).await.unwrap();
    let copy = file.with_extension("copy.zst");
    std::fs::copy(&file, &copy).unwrap();
    let (x, y) = tokio::join!(agentvm::app::backups::import(&ctx, &file), agentvm::app::backups::import(&ctx, &copy));
    let (x, y) = (x.unwrap(), y.unwrap());
    assert_ne!(x.id, y.id);
    for m in [x, y] {
        let dir = home.path().join("snapshots").join(m.id.as_str());
        assert_eq!(std::fs::read(dir.join("disk.raw")).unwrap().len(), 4 << 20);
    }
}

/// Temporary files left by a crash or a failed transfer are removed when the server starts.
#[test]
fn leftover_temporary_files_are_cleaned_up_at_start() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("tmp")).unwrap();
    std::fs::write(home.path().join("tmp/upload-ab12.tar.zst"), b"partial").unwrap();
    std::fs::create_dir_all(home.path().join("snapshots/.import-77")).unwrap();
    let id = stored_snapshot(home.path());
    agentvm::app::backups::remove_leftovers(&ctx(home.path()));
    assert!(!home.path().join("tmp/upload-ab12.tar.zst").exists());
    assert!(!home.path().join("snapshots/.import-77").exists());
    assert!(home.path().join("snapshots").join(id.as_str()).join("disk.raw").exists(), "a real snapshot was removed");
}

fn git_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        assert!(std::process::Command::new("git").arg("-C").arg(dir.path()).args(args).status().unwrap().success())
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "one"]);
    dir
}

/// Twenty VMs on the same repository pack it once: later launches get an instant copy.
#[test]
fn launches_share_one_bundle_per_repository_state() {
    use agentvm::app::bundles::prepare;
    let home = tempfile::tempdir().unwrap();
    let repo = git_repo();
    let (a, b) = (home.path().join("a.bundle"), home.path().join("b.bundle"));
    assert!(!prepare(home.path(), repo.path(), &a).unwrap(), "the first launch packs the repo");
    assert!(prepare(home.path(), repo.path(), &b).unwrap(), "the second one reuses it");
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    // New commits make a new bundle.
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "two"])
            .status()
            .unwrap()
            .success()
    );
    let c = home.path().join("c.bundle");
    assert!(!prepare(home.path(), repo.path(), &c).unwrap());
    assert_ne!(std::fs::read(&a).unwrap(), std::fs::read(&c).unwrap());
}

/// A long automatic task must not grow its event history without limit.
#[tokio::test]
async fn event_history_is_bounded_and_keeps_numbering() {
    let log = EventLog::new();
    for n in 0..25_000 {
        log.push(agent(n));
    }
    let (history, _) = log.subscribe();
    assert!(history.len() <= 10_000, "{}", history.len());
    assert_eq!(history.last().unwrap().0, 25_000);
    log.push(agent(1));
    let mut s = std::pin::pin!(log.stream());
    let first = s.next().await.unwrap().0;
    assert_eq!(first, 25_001 - history.len() as u64 + 1, "the oldest kept event");
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

/// VMs found running after a restart take their slots before queued tasks may start.
#[test]
fn running_vms_take_their_slots_without_waiting() {
    let s = agentvm::app::scheduler::Scheduler::new(1);
    let first = s.try_acquire();
    assert!(first.is_some());
    assert!(s.try_acquire().is_none(), "a second VM got the only slot");
    drop(first);
    assert!(s.try_acquire().is_some());
}
