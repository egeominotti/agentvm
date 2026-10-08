//! Histories next to a job: JSON lines appended and read back, never through a symlink.

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
