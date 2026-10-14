//! Large compacted histories still recover cursors and disclose bounded-read gaps.
use agentvm::adapters::tail::{Cursor, tail_records};
use agentvm::app::events::{EventLog, StreamItem};
use agentvm::domain::agent_event::AgentEvent;
use futures::StreamExt;

#[tokio::test]
async fn compacted_output_over_32_mib_recovers_last_sequence_and_source_cursor() {
    let home = tempfile::tempdir().unwrap();
    let journal = home.path().join("output.jsonl");
    let stream = home.path().join("stream.jsonl");
    std::fs::write(&stream, b"checkpoint\n").unwrap();
    let (_, stop) = tokio::sync::watch::channel(true);
    let records = tail_records(stream, Cursor::default(), stop).collect::<Vec<_>>().await;
    let checkpoint = records[0].cursor.clone();
    let log = EventLog::persistent(journal.clone());
    // JSON escaping expands valid 16 KiB text events to about 96 KiB each.
    for _ in 0..500 {
        log.push(StreamItem::Agent(AgentEvent::Text { text: "\0".repeat(16 * 1024) }));
    }
    log.publish_agent_line(vec![AgentEvent::Text { text: "last output".into() }], checkpoint.clone());
    log.compact(500);
    assert!(std::fs::metadata(&journal).unwrap().len() > 32 * 1024 * 1024);
    let recovered = EventLog::persistent(journal);
    assert_eq!(recovered.source_cursor(), Some(checkpoint));
    let last = recovered.page(500, 100);
    assert_eq!(last.items.len(), 1);
    assert_eq!(last.items[0].seq, 501);
    assert!(matches!(&last.items[0].item,StreamItem::Agent(AgentEvent::Text{text}) if text=="last output"));
    assert_eq!(last.next_cursor, 501);
    assert_eq!(last.dropped, 0);
    let history = recovered.page(0, 500);
    assert!(history.dropped > 0, "the bounded reader's omitted prefix must be explicit");
    assert_eq!(history.next_cursor, 501);
    recovered.push(StreamItem::Agent(AgentEvent::Retry { attempt: 1 }));
    assert_eq!(recovered.page(501, 100).items[0].seq, 502);
}
