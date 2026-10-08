//! Store ed eventi: logica in memoria, testata direttamente.

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
        Prompt::new("fai qualcosa".into()).unwrap(),
        CommitSha::parse(&"b".repeat(40)).unwrap(),
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
        .filter_map(|(_, i)| match i { StreamItem::State(s) => Some(s), _ => None })
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
