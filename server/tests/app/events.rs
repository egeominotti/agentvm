//! Event logs: history then live, bounded.

use std::time::Duration;

use agentvm::app::events::{EventLog, StreamItem};
use agentvm::domain::agent_event::AgentEvent;
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

/// A finished task keeps only its last events: hundreds of finished tasks stay cheap.
#[test]
fn a_finished_task_keeps_its_last_events_only() {
    use agentvm::app::store::Store;
    use agentvm::domain::outcome::Final;
    use agentvm::domain::task::TaskEvent;
    let repo = tempfile::tempdir().unwrap();
    let store = Store::new();
    let rec = crate::helpers::record(&repo);
    let id = rec.id.clone();
    store.insert(rec);
    let log = store.log(&id).unwrap();
    for n in 0..3000 {
        log.push(agent(n));
    }
    for e in [TaskEvent::SlotAcquired, TaskEvent::Prepared, TaskEvent::VmStarted, TaskEvent::VmExited] {
        store.apply(&id, e).unwrap();
    }
    store.apply(&id, TaskEvent::Finished(Final::NoChanges, id.branch())).unwrap();
    let (history, _) = log.subscribe();
    assert!(history.len() <= agentvm::app::events::FINISHED_HISTORY, "{}", history.len());
    assert!(matches!(history.last(), Some((_, StreamItem::State(_)))), "the final state is kept");
}
