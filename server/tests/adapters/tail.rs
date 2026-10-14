//! Following the guest's event stream as it grows.

use std::time::Duration;

use agentvm::adapters::tail::tail_lines;
use futures::StreamExt;

#[tokio::test]
async fn tail_ignores_a_stream_that_is_a_symlink() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("secret.txt");
    std::fs::write(&target, "line from the Mac\n").unwrap();
    let path = tmp.path().join("stream.jsonl");
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let lines = tokio::spawn(tail_lines(path, stop_rx).collect::<Vec<String>>());
    tokio::time::sleep(Duration::from_millis(300)).await;
    stop_tx.send(true).unwrap();
    let got = tokio::time::timeout(Duration::from_secs(5), lines).await.unwrap().unwrap();
    assert!(got.is_empty(), "{got:?}");
}

#[tokio::test]
async fn tail_follows_a_growing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("stream.jsonl");
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let lines = tokio::spawn(tail_lines(path.clone(), stop_rx).collect::<Vec<String>>());

    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!("for i in 1 2 3; do echo l$i >> {0}; sleep 0.3; done; printf tail >> {0}", path.display()))
        .spawn()
        .unwrap();
    child.wait().await.unwrap();
    stop_tx.send(true).unwrap();
    let got = tokio::time::timeout(Duration::from_secs(5), lines).await.unwrap().unwrap();
    assert_eq!(got, ["l1", "l2", "l3", "tail"]);
}

/// After a restart the whole stream of a long task is read again: splitting it must be linear.
#[tokio::test]
async fn tail_reads_a_large_stream_quickly() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("stream.jsonl");
    let line = format!("{{\"type\":\"assistant\",\"text\":\"{}\"}}\n", "x".repeat(200));
    std::fs::write(&path, line.repeat(100_000)).unwrap(); // ~21 MB
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    stop_tx.send(true).unwrap();
    let t0 = std::time::Instant::now();
    let n = tail_lines(path, stop_rx).count().await;
    assert_eq!(n, 100_000);
    assert!(t0.elapsed() < Duration::from_secs(2), "took {:?}", t0.elapsed());
}

/// A guest writing a gigantic line (by mistake or on purpose) cannot make the server hold it in
/// memory: the line is dropped with a short notice, and what follows is read as usual.
#[tokio::test]
async fn an_oversized_line_is_skipped_not_held_in_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("stream.jsonl");
    let mut content = b"first\n".to_vec();
    content.extend(std::iter::repeat_n(b'x', 20 << 20));
    content.extend_from_slice(b"\nafter\n");
    std::fs::write(&path, content).unwrap();
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let lines = tokio::spawn(tail_lines(path, stop_rx).collect::<Vec<String>>());
    tokio::time::sleep(Duration::from_millis(500)).await;
    stop_tx.send(true).unwrap();
    let got = tokio::time::timeout(Duration::from_secs(10), lines).await.unwrap().unwrap();
    assert_eq!(got.len(), 3, "{:?}", got.iter().map(|l| l.len()).collect::<Vec<_>>());
    assert_eq!(got[0], "first");
    assert!(got[1].len() < 200 && got[1].contains("skipped"), "{}", &got[1][..got[1].len().min(200)]);
    assert_eq!(got[2], "after");
}

#[tokio::test]
async fn tail_resumes_complete_bytes_and_retains_legitimate_repeated_text() {
    use agentvm::adapters::tail::{Cursor, tail_records};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("stream.jsonl");
    std::fs::write(&path, b"same\n").unwrap();
    let (_, stop) = tokio::sync::watch::channel(true);
    let first = tail_records(path.clone(), Cursor::default(), stop).collect::<Vec<_>>().await;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].cursor.offset, 5);
    use std::io::Write;
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"same\n").unwrap();
    let (_, stop) = tokio::sync::watch::channel(true);
    let second = tail_records(path, first[0].cursor.clone(), stop).collect::<Vec<_>>().await;
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].line, "same");
    assert_eq!(second[0].cursor.offset, 10);
}

#[tokio::test]
async fn tail_resets_for_rewritten_or_replaced_files_even_when_the_new_file_is_longer() {
    use agentvm::adapters::tail::{Cursor, tail_records};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("stream.jsonl");
    std::fs::write(&path, b"old\n").unwrap();
    let (_, stop) = tokio::sync::watch::channel(true);
    let old = tail_records(path.clone(), Cursor::default(), stop).collect::<Vec<_>>().await;
    std::fs::write(&path, b"rewritten-longer\n").unwrap();
    let (_, stop) = tokio::sync::watch::channel(true);
    let rewritten = tail_records(path.clone(), old[0].cursor.clone(), stop).collect::<Vec<_>>().await;
    assert_eq!(rewritten[0].line, "rewritten-longer");
    let replacement = tmp.path().join("replacement");
    std::fs::write(&replacement, b"replacement-longer-than-before\n").unwrap();
    std::fs::rename(replacement, &path).unwrap();
    let (_, stop) = tokio::sync::watch::channel(true);
    let replaced = tail_records(path, rewritten[0].cursor.clone(), stop).collect::<Vec<_>>().await;
    assert_eq!(replaced[0].line, "replacement-longer-than-before");
}

#[tokio::test]
async fn tail_does_not_checkpoint_an_unfinished_line() {
    use agentvm::adapters::tail::{Cursor, tail_records};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("stream.jsonl");
    std::fs::write(&path, b"complete\npartial").unwrap();
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let records = tail_records(path, Cursor::default(), stop_rx);
    let mut records = std::pin::pin!(records);
    let first = records.next().await.unwrap();
    assert_eq!(first.cursor.offset, 9);
    assert!(tokio::time::timeout(Duration::from_millis(100), records.next()).await.is_err());
    stop_tx.send(true).unwrap();
    assert_eq!(records.next().await.unwrap().line, "partial");
}

#[tokio::test]
async fn a_live_follower_detects_a_longer_rewrite_before_reading_new_bytes() {
    use agentvm::adapters::tail::{Cursor, tail_records};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("stream.jsonl");
    std::fs::write(&path, b"old\n").unwrap();
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let records = tail_records(path.clone(), Cursor::default(), stop_rx);
    let mut records = std::pin::pin!(records);
    assert_eq!(records.next().await.unwrap().line, "old");
    std::fs::write(&path, b"new-and-longer\n").unwrap();
    let record = tokio::time::timeout(Duration::from_secs(2), records.next()).await.unwrap().unwrap();
    assert_eq!(record.line, "new-and-longer");
    stop_tx.send(true).unwrap();
}
