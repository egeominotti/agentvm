//! The server's own log: JSON lines, one file a day; read back per VM for its diagnostics.

use agentvm::adapters::server_log::lines_for;

fn line(task: &str, message: &str) -> String {
    format!(
        r#"{{"timestamp":"2026-10-09T10:00:00Z","level":"INFO","fields":{{"message":"{message}","task":"{task}"}},"target":"agentvm"}}"#
    )
}

#[test]
fn a_vms_lines_come_back_in_order_from_the_last_two_days() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("agentvm.log.2026-10-07"), line("A", "too old") + "\n").unwrap();
    std::fs::write(dir.path().join("agentvm.log.2026-10-08"), line("A", "yesterday") + "\n").unwrap();
    let today = [line("A", "first"), line("B", "other vm"), "not json".into(), line("A", "second")].join("\n");
    std::fs::write(dir.path().join("agentvm.log.2026-10-09"), today + "\n").unwrap();
    let got = lines_for(dir.path(), "A", 10);
    assert_eq!(got.len(), 3, "{got:?}");
    assert!(got[0].contains("yesterday") && got[1].contains("first") && got[2].contains("second"), "{got:?}");
    assert_eq!(lines_for(dir.path(), "A", 1).len(), 1, "only the newest when capped");
    assert!(lines_for(dir.path(), "A", 1)[0].contains("second"));
}

/// A day of logs with many VMs can be large: only its end is read.
#[test]
fn a_large_log_is_read_from_its_end_only() {
    let dir = tempfile::tempdir().unwrap();
    let filler = line("B", &"x".repeat(1000)) + "\n";
    let mut big = filler.repeat(20_000);
    big.push_str(&(line("A", "at the end") + "\n"));
    std::fs::write(dir.path().join("agentvm.log.2026-10-09"), big).unwrap();
    let t0 = std::time::Instant::now();
    let got = lines_for(dir.path(), "A", 10);
    assert_eq!(got.len(), 1);
    assert!(t0.elapsed() < std::time::Duration::from_secs(1), "{:?}", t0.elapsed());
}

#[test]
fn no_log_folder_means_no_lines() {
    assert!(lines_for(std::path::Path::new("/nonexistent/logs"), "A", 10).is_empty());
}

/// What the server logs with `tracing` lands, as JSON with the VM's id, where the diagnostics
/// read it.
#[test]
fn logged_events_are_found_by_their_vm() {
    let dir = tempfile::tempdir().unwrap();
    let guard = agentvm::logging::init(dir.path(), "info").expect("logging set up");
    tracing::info!(task = "0199c4b6-a2f2-7fff-bfff-ffffffffffff", from = "queued", to = "preparing", "state changed");
    tracing::debug!(task = "0199c4b6-a2f2-7fff-bfff-ffffffffffff", "below the level");
    drop(guard); // flushes the background writer
    let got = lines_for(dir.path(), "0199c4b6-a2f2-7fff-bfff-ffffffffffff", 10);
    assert_eq!(got.len(), 1, "{got:?}");
    let v: serde_json::Value = serde_json::from_str(&got[0]).unwrap();
    assert_eq!(v["level"], "INFO");
    assert_eq!(v["fields"]["message"], "state changed");
    assert_eq!(v["fields"]["to"], "preparing");
}
