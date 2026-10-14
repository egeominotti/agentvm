//! External commands run with a time limit: a hung one is killed, never waited on forever.

use std::process::Command;
use std::time::{Duration, Instant};

use agentvm::process::output;

#[test]
fn a_command_that_finishes_gives_its_output() {
    let out =
        output(Command::new("sh").args(["-c", "echo out; echo err >&2; exit 3"]), Duration::from_secs(5)).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "out\n");
    assert_eq!(String::from_utf8_lossy(&out.stderr), "err\n");
    assert_eq!(out.status.code(), Some(3));
}

/// A git waiting on a lock, a Keychain waiting for an unlock dialog: killed at the limit.
#[test]
fn a_hung_command_is_killed_at_its_limit() {
    let t0 = Instant::now();
    let err = output(Command::new("sleep").arg("30"), Duration::from_millis(300)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert!(err.to_string().contains("sleep"), "{err}");
    assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
}

#[test]
fn inherited_output_pipes_cannot_bypass_the_time_limit() {
    let t0 = Instant::now();
    let err = output(Command::new("sh").args(["-c", "sleep 2 & printf done"]), Duration::from_millis(200)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert!(t0.elapsed() < Duration::from_secs(1), "{:?}", t0.elapsed());
}

#[test]
fn a_timeout_also_stops_children_that_inherited_the_output_pipes() {
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if let Ok(pid) = std::fs::read_to_string(&self.0) {
                let _ = Command::new("kill").args(["-KILL", pid.trim()]).output();
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let cleanup = Cleanup(dir.path().join("child.pid"));
    let err = output(
        Command::new("sh")
            .args(["-c", "sleep 30 & printf '%s\\n' \"$!\" > \"$TASK_PID_FILE\""])
            .env("TASK_PID_FILE", &cleanup.0),
        Duration::from_millis(200),
    )
    .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    let pid = std::fs::read_to_string(&cleanup.0).unwrap();
    let alive = Command::new("kill").args(["-0", pid.trim()]).output().unwrap().status.success();
    assert!(!alive, "the timed-out command left its child and output readers alive");
}

/// Output larger than a pipe's buffer must not deadlock the wait.
#[test]
fn large_output_does_not_block() {
    let out = output(Command::new("sh").args(["-c", "head -c 1000000 /dev/zero"]), Duration::from_secs(5)).unwrap();
    assert_eq!(out.stdout.len(), 1_000_000);
}

/// Recognising agentvm's VM helper by its pid asks the kernel, not `ps`: no process to spawn, so
/// a busy Mac at restart never mistakes a running VM for a dead one.
#[test]
fn a_pid_is_recognised_as_a_vm_helper_without_spawning_anything() {
    use agentvm::pids::{HelperPid, helper_pid};
    let t0 = std::time::Instant::now();
    assert_eq!(helper_pid(std::process::id()), HelperPid::Other);
    assert_eq!(helper_pid(999_999), HelperPid::Gone);
    assert!(t0.elapsed() < std::time::Duration::from_millis(50), "{:?}", t0.elapsed());
}

/// A quick command costs what it takes, not a polling interval: the repository check runs
/// several git commands while the user types.
#[test]
fn a_quick_command_returns_at_once() {
    // The median: a busy Mac (tests in parallel) slows a few runs, never most of them. Polling
    // every 20 ms put every run at 20 ms or more.
    let mut runs: Vec<Duration> = (0..21)
        .map(|_| {
            let t0 = Instant::now();
            output(&mut Command::new("true"), Duration::from_secs(5)).unwrap();
            t0.elapsed()
        })
        .collect();
    runs.sort();
    assert!(runs[10] < Duration::from_millis(15), "median run {:?} (all: {runs:?})", runs[10]);
}
