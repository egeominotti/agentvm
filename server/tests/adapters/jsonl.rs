//! Histories next to a job: JSON lines appended and read back, never through a symlink.

use std::io::Write;

use agentvm::domain::telemetry::TelemetrySample;
use agentvm::jsonl::{append, read};

fn at(t: f64) -> TelemetrySample {
    TelemetrySample { at: t, cpu_pct: 1.0, ..Default::default() }
}

#[test]
fn lines_are_appended_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("telemetry.jsonl");
    for t in [10.0, 20.0, 30.0] {
        append(&path, &at(t)).unwrap();
    }
    let back: Vec<TelemetrySample> = read(&path);
    assert_eq!(back.iter().map(|s| s.at).collect::<Vec<_>>(), [10.0, 20.0, 30.0]);
    assert!(read::<TelemetrySample>(&dir.path().join("none.jsonl")).is_empty());
}

#[test]
fn a_symlink_in_its_place_is_never_followed() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("elsewhere.txt");
    std::fs::write(&target, "untouched").unwrap();
    let path = dir.path().join("telemetry.jsonl");
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert!(append(&path, &at(1.0)).is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "untouched");
}

/// A line cut short (a full disk, a power cut) loses only itself: the next line is not glued to
/// it, and garbage bytes do not make the whole history unreadable.
#[test]
fn a_partial_line_costs_only_itself() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("t.jsonl");
    agentvm::jsonl::append(&file, &at(1.0)).unwrap();
    // What a write interrupted half way leaves: no newline, and bytes that are not UTF-8.
    std::fs::OpenOptions::new().append(true).open(&file).unwrap().write_all(b"{\"at\":2.0,\xff\xfe").unwrap();
    agentvm::jsonl::append(&file, &at(3.0)).unwrap();
    let back: Vec<TelemetrySample> = agentvm::jsonl::read(&file);
    assert_eq!(back.iter().map(|s| s.at).collect::<Vec<_>>(), vec![1.0, 3.0]);
}

/// The last value only, read from the end: picking up after a restart costs the same for a day
/// of history as for a minute.
#[test]
fn the_last_value_is_read_from_the_end() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("t.jsonl");
    assert!(agentvm::jsonl::last::<TelemetrySample>(&file).is_none());
    for n in 0..5000 {
        append(&file, &at(f64::from(n))).unwrap();
    }
    assert_eq!(agentvm::jsonl::last::<TelemetrySample>(&file).map(|s| s.at), Some(4999.0));
}
