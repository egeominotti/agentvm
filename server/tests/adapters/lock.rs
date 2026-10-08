//! AGENTVM_HOME: private to its owner and held by one server at a time.

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

/// Repos, VM disks and job folders live in AGENTVM_HOME: no other user of the Mac may enter it.
#[test]
fn agentvm_home_is_private_to_its_owner() {
    use agentvm::adapters::lock::InstanceLock;
    let tmp = tempfile::tempdir().unwrap();
    let fresh = tmp.path().join("fresh");
    let _a = InstanceLock::acquire(&fresh).unwrap();
    assert_eq!(std::fs::metadata(&fresh).unwrap().permissions().mode() & 0o777, 0o700);
    let open = tmp.path().join("open");
    std::fs::create_dir(&open).unwrap();
    std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _b = InstanceLock::acquire(&open).unwrap();
    assert_eq!(std::fs::metadata(&open).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
fn instance_lock_is_exclusive_until_dropped() {
    use agentvm::adapters::lock::InstanceLock;
    let tmp = tempfile::tempdir().unwrap();
    let first = InstanceLock::acquire(tmp.path()).unwrap();
    assert!(InstanceLock::acquire(tmp.path()).is_err());
    drop(first);
    // A child spawned at that moment by another test may hold the descriptor until its exec.
    let t0 = std::time::Instant::now();
    while InstanceLock::acquire(tmp.path()).is_err() {
        assert!(t0.elapsed() < Duration::from_secs(2), "lock not released");
        std::thread::sleep(Duration::from_millis(20));
    }
}
