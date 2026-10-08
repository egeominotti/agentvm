//! From a failure to what to do about it.

use agentvm::domain::diagnosis::hint;

fn says(reason: &str, job_log: &str, words: &str) {
    let h = hint(reason, job_log).unwrap_or_else(|| panic!("no hint for {reason:?}"));
    assert!(h.to_lowercase().contains(&words.to_lowercase()), "{reason:?} → {h:?}");
}

#[test]
fn known_failures_come_with_what_to_do() {
    says("claude_exit 1", "[3.1s] running .agentvm/setup.sh\n[9.2s] setup failed (see setup.log)", "setup.sh");
    says("claude_install_failed: 2.1.999", "", "could not be installed");
    says("timeout", "", "time limit");
    says("vm_error: the VM stopped while agentvm was not running", "", "console");
    says("guest_error (exit 128)", "", "job log");
    says("guest_no_result", "", "job log");
    says("fetch_failed: git import failed", "", "imported");
    says("disk clone: No space left on device (os error 28)", "", "disk is full");
    says("no Claude token: run `claude setup-token`", "", "Settings");
    says("interrupted while preparing: launch it again", "", "launch it again");
    says("claude_exit 2", "", "Claude");
}

/// The disk kept as a snapshot is said on top of the cause.
#[test]
fn a_kept_disk_is_mentioned() {
    let h = hint("timeout. Its disk is kept in Snapshots as \"Interrupted: fix\"", "").unwrap();
    assert!(h.contains("time limit") && h.contains("Snapshots"), "{h}");
}

#[test]
fn an_unknown_reason_has_no_hint() {
    assert_eq!(hint("something new", ""), None);
}
