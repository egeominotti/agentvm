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
