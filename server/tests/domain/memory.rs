//! How much memory a VM may keep.

use agentvm::domain::memory::memory_target;

#[test]
fn idle_vms_give_memory_back_and_busy_ones_get_it_all() {
    // Busy, or not idle long enough: everything it was given.
    assert_eq!(memory_target(4096, 700, true, 600), 4096);
    assert_eq!(memory_target(4096, 700, false, 30), 4096);
    // Idle for a minute: what it uses plus 1 GB of headroom, in 256 MB steps, at least 1 GB.
    assert_eq!(memory_target(4096, 700, false, 61), 1792);
    assert_eq!(memory_target(4096, 100, false, 61), 1280);
    assert_eq!(memory_target(4096, 3500, false, 61), 4096);
    assert_eq!(memory_target(2048, 50, false, 3600), 1280);
}
