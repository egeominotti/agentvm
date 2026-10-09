//! Claude's session files copied out of the VM, read from where the last read stopped.

use agentvm::adapters::transcripts::{Cursor, read_new};

const PAGE: u64 = 4 << 20;

fn append(dir: &std::path::Path, name: &str, text: &str) {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(name))
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

/// Two sessions growing at once (Claude in the terminal and in the root shell): every line is
/// read once, whichever file changed last.
#[test]
fn each_line_is_read_once_while_sessions_grow() {
    let dir = tempfile::tempdir().unwrap();
    append(dir.path(), "p--a.jsonl", "a0\na1\n");
    append(dir.path(), "p--b.jsonl", "b0\n");
    let (lines, cursor) = read_new(dir.path(), &Cursor::default(), PAGE);
    assert_eq!(lines, ["a0", "a1", "b0"]);
    append(dir.path(), "p--b.jsonl", "b1\n");
    append(dir.path(), "p--a.jsonl", "a2\n");
    let (lines, cursor) = read_new(dir.path(), &cursor, PAGE);
    assert_eq!(lines, ["a2", "b1"]);
    assert!(read_new(dir.path(), &cursor, PAGE).0.is_empty());
}

/// A page holds at most `max_bytes` of lines; the next pages continue exactly after it.
#[test]
fn pages_are_bounded_in_bytes_and_continue_exactly() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..100 {
        append(dir.path(), "s.jsonl", &format!("{i:03}{}\n", "x".repeat(10_000)));
    }
    let (mut all, mut cursor, mut pages) = (Vec::new(), Cursor::default(), 0);
    loop {
        let (lines, next) = read_new(dir.path(), &cursor, 100_000);
        if lines.is_empty() {
            break;
        }
        assert!(lines.len() <= 10, "{}", lines.len());
        all.extend(lines);
        cursor = next;
        pages += 1;
    }
    assert_eq!(all.len(), 100);
    assert!(all.iter().enumerate().all(|(i, l)| l.starts_with(&format!("{i:03}"))));
    assert!(pages >= 10);
}

/// Hours of work make a big session: reading what is new costs what is new, not the file.
#[test]
fn reading_from_the_cursor_does_not_rescan_the_file() {
    let dir = tempfile::tempdir().unwrap();
    append(dir.path(), "s.jsonl", &format!("{}\n", "y".repeat(400)).repeat(120_000));
    let (_, mut cursor) = read_new(dir.path(), &Cursor::default(), u64::MAX);
    append(dir.path(), "s.jsonl", "new\n");
    let t0 = std::time::Instant::now();
    let (lines, next) = read_new(dir.path(), &cursor, PAGE);
    assert_eq!(lines, ["new"]);
    assert!(t0.elapsed() < std::time::Duration::from_millis(50), "{:?}", t0.elapsed());
    cursor = next;
    assert!(read_new(dir.path(), &cursor, PAGE).0.is_empty());
}

/// A last line still being copied is left for later; a line past 1 MB is skipped, not kept.
#[test]
fn partial_and_oversized_lines() {
    let dir = tempfile::tempdir().unwrap();
    append(dir.path(), "s.jsonl", &format!("l0\n{}\nl1-half", "z".repeat(3 << 20)));
    let (lines, cursor) = read_new(dir.path(), &Cursor::default(), u64::MAX);
    assert_eq!(lines, ["l0"]);
    append(dir.path(), "s.jsonl", " and the rest\n");
    assert_eq!(read_new(dir.path(), &cursor, PAGE).0, ["l1-half and the rest"]);
}

/// The folder is the guest's: neither a symlinked folder nor a symlinked file is read.
#[test]
fn guest_planted_symlinks_are_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let mac = dir.path().join("mac");
    std::fs::create_dir(&mac).unwrap();
    append(&mac, "secret.jsonl", "mac file\n");
    std::os::unix::fs::symlink(&mac, dir.path().join("claude")).unwrap();
    assert!(read_new(&dir.path().join("claude"), &Cursor::default(), PAGE).0.is_empty());
    let share = dir.path().join("share");
    std::fs::create_dir(&share).unwrap();
    std::os::unix::fs::symlink(mac.join("secret.jsonl"), share.join("x.jsonl")).unwrap();
    assert!(read_new(&share, &Cursor::default(), PAGE).0.is_empty());
}

/// A guest that floods its folder with empty session files cannot make every read open them
/// all, nor grow the cursor the dashboard sends back in its address.
#[test]
fn empty_session_files_do_not_grow_the_cursor() {
    let dir = tempfile::tempdir().unwrap();
    for n in 0..3000 {
        std::fs::write(dir.path().join(format!("flood-{n:05}.jsonl")), "").unwrap();
    }
    std::fs::write(dir.path().join("a-real.jsonl"), "{\"x\":1}\n").unwrap();
    let (lines, cursor) = agentvm::adapters::transcripts::read_new(dir.path(), &Default::default(), 1 << 20);
    assert_eq!(lines, vec!["{\"x\":1}".to_owned()]);
    assert_eq!(cursor.0.len(), 1, "only files with something read are remembered");
}
