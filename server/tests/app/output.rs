//! Finite output resumes by sequence number and survives a daemon restart.
use agentvm::app::events::{EventLog, StreamItem};
use agentvm::domain::agent_event::AgentEvent;

#[test]
fn output_pages_resume_without_duplicates_and_report_compacted_gaps() {
    let log = EventLog::new();
    for n in 1..=8 {
        log.push(StreamItem::Agent(AgentEvent::Retry { attempt: n }));
    }
    let first = log.page(0, 3);
    assert_eq!(first.items.iter().map(|i| i.seq).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(first.next_cursor, 3);
    assert!(first.more);
    assert_eq!(first.dropped, 0);
    log.compact(3);
    let second = log.page(first.next_cursor, 2);
    assert_eq!(second.items.iter().map(|i| i.seq).collect::<Vec<_>>(), [6, 7]);
    assert_eq!(second.dropped, 2);
    assert_eq!(second.next_cursor, 7);
    assert!(second.more);
    let last = log.page(7, 100);
    assert_eq!(last.items[0].seq, 8);
    assert!(!last.more);
    assert!(log.page(8, 100).items.is_empty());
    assert_eq!(log.page(8, 100).next_cursor, 8);
}

#[tokio::test]
async fn finished_agent_output_keeps_cursors_across_restart() {
    use agentvm::app::store::Store;
    use agentvm::domain::task::TaskEvent;
    let (home, repo) = (tempfile::tempdir().unwrap(), crate::helpers::git_repo());
    let ctx = crate::helpers::ctx(home.path());
    let record = crate::helpers::record(&repo);
    let id = record.id.clone();
    ctx.store.insert(record);
    ctx.store.log(&id).unwrap().push(StreamItem::Agent(AgentEvent::Text { text: "saved real output".into() }));
    ctx.store.apply(&id, TaskEvent::Failure("finished".into())).unwrap();
    let before = agentvm::app::output::page(&ctx, &id, 0, 100).unwrap();
    let next = crate::helpers::ctx(home.path());
    for record in Store::load(&next.config.jobs()) {
        next.store.insert(record);
    }
    let after = agentvm::app::output::page(&next, &id, 0, 100).unwrap();
    assert_eq!(serde_json::to_value(&before).unwrap(), serde_json::to_value(&after).unwrap());
    assert!(agentvm::app::output::page(&next, &id, after.next_cursor, 100).unwrap().items.is_empty());
}

#[tokio::test]
async fn legacy_finished_stream_is_parsed_and_only_imported_once() {
    use agentvm::domain::task::TaskEvent;
    let (home, repo) = (tempfile::tempdir().unwrap(), crate::helpers::git_repo());
    let record = crate::helpers::record(&repo);
    let id = record.id.clone();
    let ctx = crate::helpers::ctx(home.path());
    ctx.store.insert(record);
    ctx.store.apply(&id, TaskEvent::Failure("finished".into())).unwrap();
    let share = ctx.config.jobs().join(id.as_str()).join("share");
    std::fs::create_dir_all(&share).unwrap();
    std::fs::write(
        share.join("stream.jsonl"),
        b"{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"legacy output\"}]}}\n",
    )
    .unwrap();
    let first = agentvm::app::output::page(&ctx, &id, 0, 100).unwrap();
    assert!(
        first
            .items
            .iter()
            .any(|i| matches!(&i.item,StreamItem::Agent(AgentEvent::Text {text}) if text=="legacy output"))
    );
    assert!(agentvm::app::output::page(&ctx, &id, first.next_cursor, 100).unwrap().items.is_empty());
}

#[tokio::test]
async fn recovered_raw_output_redacts_the_real_host_credential_before_persistence() {
    use agentvm::domain::{agents::AgentKind, providers::Provider, task::TaskEvent};
    use std::process::Command;
    let (home, repo) = (tempfile::tempdir().unwrap(), crate::helpers::git_repo());
    let path = home.path().join("none.keychain-db");
    assert!(Command::new("security").args(["create-keychain", "-p", "x"]).arg(&path).status().unwrap().success());
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
        }
    }
    let _cleanup = Cleanup(path);
    let ctx = crate::helpers::ctx(home.path());
    let token = "recovered-output-host-secret-123";
    ctx.keychain.write_provider_key(Provider::Custom, &agentvm::secret::Secret::new(token.into())).unwrap();
    let mut record = crate::helpers::record(&repo);
    record.agent.kind = AgentKind::Codex;
    record.agent.provider = Some(Provider::Custom);
    let id = record.id.clone();
    ctx.store.insert(record);
    ctx.store.apply(&id, TaskEvent::Failure("finished".into())).unwrap();
    let share = ctx.config.jobs().join(id.as_str()).join("share");
    std::fs::create_dir_all(&share).unwrap();
    let line = serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":format!("safe {token} output")}]}});
    std::fs::write(share.join("stream.jsonl"), format!("{line}\n")).unwrap();
    let page = agentvm::app::output::page(&ctx, &id, 0, 100).unwrap();
    let text = serde_json::to_string(&page).unwrap();
    assert!(text.contains("safe [REDACTED] output"));
    assert!(!text.contains(token));
    assert!(
        !std::fs::read_to_string(ctx.config.jobs().join(id.as_str()).join("output.jsonl")).unwrap().contains(token)
    );
}

#[tokio::test]
async fn legacy_output_reads_only_a_bounded_suffix_and_reports_the_gap() {
    use agentvm::domain::task::TaskEvent;
    use std::io::{Seek, SeekFrom, Write};
    let (home, repo) = (tempfile::tempdir().unwrap(), crate::helpers::git_repo());
    let ctx = crate::helpers::ctx(home.path());
    let record = crate::helpers::record(&repo);
    let id = record.id.clone();
    ctx.store.insert(record);
    ctx.store.apply(&id, TaskEvent::Failure("finished".into())).unwrap();
    let share = ctx.config.jobs().join(id.as_str()).join("share");
    std::fs::create_dir_all(&share).unwrap();
    let mut file = std::fs::File::create(share.join("stream.jsonl")).unwrap();
    file.seek(SeekFrom::Start(16 << 30)).unwrap();
    file.write_all(b"\n{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"bounded tail output\"}]}}\n").unwrap();
    let page = agentvm::app::output::page(&ctx, &id, 0, 100).unwrap();
    assert!(page.dropped > 0);
    assert!(
        page.items
            .iter()
            .any(|i| matches!(&i.item,StreamItem::Agent(AgentEvent::Text{text}) if text=="bounded tail output"))
    );
}

#[tokio::test]
async fn published_line_and_byte_cursor_restart_together_without_duplicate_events() {
    use agentvm::adapters::tail::{Cursor, tail_records};
    use futures::StreamExt;
    use std::io::Write;
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("stream.jsonl");
    let journal = home.path().join("output.jsonl");
    let line=b"{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"same\"},{\"type\":\"text\",\"text\":\"same\"}]}}\n";
    std::fs::write(&path, line).unwrap();
    let log = EventLog::persistent(journal.clone());
    let (_, stop) = tokio::sync::watch::channel(true);
    for record in tail_records(path.clone(), Cursor::default(), stop).collect::<Vec<_>>().await {
        log.publish_agent_line(agentvm::domain::agent_event::parse_line(&record.line), record.cursor);
    }
    assert_eq!(log.page(0, 100).items.len(), 2);
    std::fs::OpenOptions::new().append(true).open(&journal).unwrap().write_all(b"{\"items\":[").unwrap();
    let restored = EventLog::persistent(journal);
    assert_eq!(restored.page(0, 100).items.len(), 2);
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(line).unwrap();
    let (_, stop) = tokio::sync::watch::channel(true);
    let new = tail_records(path, restored.source_cursor().unwrap(), stop).collect::<Vec<_>>().await;
    assert_eq!(new.len(), 1);
    restored.publish_agent_line(agentvm::domain::agent_event::parse_line(&new[0].line), new[0].cursor.clone());
    let page = restored.page(0, 100);
    assert_eq!(page.items.iter().map(|i| i.seq).collect::<Vec<_>>(), [1, 2, 3, 4]);
    assert!(page.items.iter().all(|i| matches!(&i.item,StreamItem::Agent(AgentEvent::Text{text}) if text=="same")));
}
