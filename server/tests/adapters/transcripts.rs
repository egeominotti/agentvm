//! Claude's session files copied out of the VM, read a page at a time.

use agentvm::adapters::transcripts::{Page, read_page};

fn session(dir: &std::path::Path, name: &str, lines: &[&str]) {
    std::fs::write(dir.join(name), lines.join("\n") + "\n").unwrap();
}

#[test]
fn lines_are_paged_across_session_files_in_order() {
    let dir = tempfile::tempdir().unwrap();
    session(dir.path(), "a--1.jsonl", &["l0", "l1", "l2"]);
    std::thread::sleep(std::time::Duration::from_millis(20));
    session(dir.path(), "a--2.jsonl", &["l3", "l4"]);
    let Page { lines, next } = read_page(dir.path(), 0, 4);
    assert_eq!(lines, ["l0", "l1", "l2", "l3"]);
    assert_eq!(next, 4);
    let rest = read_page(dir.path(), next, 4);
    assert_eq!((rest.lines, rest.next), (vec!["l4".to_owned()], 5));
    assert!(read_page(dir.path(), 5, 4).lines.is_empty());
    assert!(read_page(&dir.path().join("none"), 0, 4).lines.is_empty());
}

/// Hours of work make a big session: a page never loads the whole file, and a gigantic line
/// is skipped rather than held in memory.
#[test]
fn a_big_session_is_paged_in_bounded_memory() {
    let dir = tempfile::tempdir().unwrap();
    let mut big = String::new();
    for i in 0..200_000 {
        big.push_str(&format!("{{\"n\":{i},\"pad\":\"{}\"}}\n", "x".repeat(400)));
    }
    big.push_str(&"y".repeat(30 << 20));
    big.push_str("\nlast\n");
    std::fs::write(dir.path().join("s.jsonl"), big).unwrap();
    let t0 = std::time::Instant::now();
    let page = read_page(dir.path(), 199_998, 10);
    assert_eq!(page.lines.len(), 3, "two normal lines, the huge one skipped, then the last");
    assert!(page.lines[2] == "last", "{:?}", &page.lines[2][..10.min(page.lines[2].len())]);
    assert!(t0.elapsed() < std::time::Duration::from_secs(3), "{:?}", t0.elapsed());
}

/// The folder is the guest's: a symlink it planted there is never read.
#[test]
fn a_planted_symlink_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("secret"), "mac file\n").unwrap();
    let share = dir.path().join("claude");
    std::fs::create_dir(&share).unwrap();
    std::os::unix::fs::symlink(dir.path().join("secret"), share.join("x.jsonl")).unwrap();
    assert!(read_page(&share, 0, 10).lines.is_empty());
}

/// The guest copies a session as it grows: a last line still being written is not a line yet,
/// and once complete it is read, not skipped.
#[test]
fn a_line_still_being_copied_is_read_once_complete() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("s.jsonl"), "l0\nl1-half").unwrap();
    let first = read_page(dir.path(), 0, 10);
    assert_eq!((first.lines.clone(), first.next), (vec!["l0".to_owned()], 1));
    std::fs::write(dir.path().join("s.jsonl"), "l0\nl1-half and the rest\n").unwrap();
    assert_eq!(read_page(dir.path(), first.next, 10).lines, ["l1-half and the rest"]);
}
