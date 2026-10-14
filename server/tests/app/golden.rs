//! Rebuilding the VM image from the dashboard: a stuck build ends, never "running" forever.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use agentvm::app::golden::GoldenService;

/// A build script that hangs is stopped at the limit, with what it started (the VM it boots).
#[tokio::test]
async fn a_stuck_rebuild_is_stopped_at_its_time_limit() {
    let home = tempfile::tempdir().unwrap();
    let script = home.path().join("build-golden.sh");
    let child_pid = home.path().join("child.pid");
    let builder_pid = home.path().join("builder.pid");
    std::fs::write(
        &script,
        "#!/bin/sh\nif [ \"${1:-}\" = --prepare ]; then exit 0; fi\necho $$ > \"$AGENTVM_HOME/builder.pid\"\nsleep 300 &\necho $! > \"$AGENTVM_HOME/child.pid\"\nwait\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut cleanup = RebuildCleanup { builder_pid, child_pid: child_pid.clone(), armed: true };
    // Assess the real temporary executable before starting the short rebuild deadline.
    let mut prepare = tokio::process::Command::new(&script)
        .arg("--prepare")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(10), prepare.wait())
            .await
            .expect("the executable did not prepare")
            .unwrap()
            .success()
    );
    assert!(!child_pid.exists(), "preparation must not start the rebuild's child");
    let golden = GoldenService::new(home.path().to_path_buf(), script).with_time_limit(Duration::from_millis(500));
    golden.rebuild("stable").unwrap();
    let pid = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(pid) = read_pid(&child_pid) {
                break pid;
            }
            let status = golden.status();
            assert!(
                status.rebuilding,
                "the rebuild stopped before its child started: {:?}; {}",
                status.last_result, status.log_tail
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the build's real child did not start");
    for _ in 0..50 {
        if !golden.status().rebuilding {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let status = golden.status();
    assert!(!status.rebuilding, "still rebuilding");
    assert!(status.last_result.as_deref().unwrap_or_default().contains("longer than"), "{:?}", status.last_result);
    let alive = Command::new("kill").args(["-0", &pid.to_string()]).status().unwrap().success();
    assert!(!alive, "what the build started is still running");
    cleanup.armed = false;
}

fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse::<u32>().ok().filter(|pid| *pid > 0)
}

struct RebuildCleanup {
    builder_pid: PathBuf,
    child_pid: PathBuf,
    armed: bool,
}

impl Drop for RebuildCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        for target in [
            read_pid(&self.builder_pid).map(|pid| format!("-{pid}")),
            read_pid(&self.child_pid).map(|pid| pid.to_string()),
        ]
        .into_iter()
        .flatten()
        {
            let _ = Command::new("kill")
                .args(["-KILL", &target])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}
