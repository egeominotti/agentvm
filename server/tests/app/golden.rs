//! Rebuilding the VM image from the dashboard: a stuck build ends, never "running" forever.

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use agentvm::app::golden::GoldenService;

/// A build script that hangs is stopped at the limit, with what it started (the VM it boots).
#[tokio::test]
async fn a_stuck_rebuild_is_stopped_at_its_time_limit() {
    let home = tempfile::tempdir().unwrap();
    let script = home.path().join("build-golden.sh");
    let child_pid = home.path().join("child.pid");
    std::fs::write(&script, format!("#!/bin/sh\nsleep 300 &\necho $! > {}\nwait\n", child_pid.display())).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let golden = GoldenService::new(home.path().to_path_buf(), script).with_time_limit(Duration::from_millis(500));
    golden.rebuild("stable").unwrap();
    for _ in 0..50 {
        if !golden.status().rebuilding {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let status = golden.status();
    assert!(!status.rebuilding, "still rebuilding");
    assert!(status.last_result.as_deref().unwrap_or_default().contains("longer than"), "{:?}", status.last_result);
    let pid = std::fs::read_to_string(&child_pid).unwrap();
    let alive = std::process::Command::new("kill").args(["-0", pid.trim()]).status().unwrap().success();
    assert!(!alive, "what the build started is still running");
}
