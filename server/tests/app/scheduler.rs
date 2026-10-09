//! VM slots: resized at runtime, taken first by VMs found after a restart, and never more
//! memory than this Mac can give.

use std::time::Duration;

use agentvm::app::scheduler::Scheduler;

const GB: u64 = 1024;
const BUDGET: u64 = 56 * GB;

#[tokio::test]
async fn scheduler_grows_and_shrinks_at_runtime() {
    let s = Scheduler::new(1);
    let a = s.acquire(GB, BUDGET).await;
    assert!(tokio::time::timeout(Duration::from_millis(50), s.acquire(GB, BUDGET)).await.is_err(), "only 1 slot");
    s.resize(2);
    let b =
        tokio::time::timeout(Duration::from_millis(200), s.acquire(GB, BUDGET)).await.expect("second slot after grow");
    assert_eq!(s.concurrency(), 2);
    s.resize(1);
    drop(a);
    drop(b);
    let _c = s.acquire(GB, BUDGET).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(tokio::time::timeout(Duration::from_millis(100), s.acquire(GB, BUDGET)).await.is_err(), "back to 1 slot");
}

/// VMs found running after a restart take their slots before queued tasks may start.
#[test]
fn running_vms_take_their_slots_without_waiting() {
    let s = Scheduler::new(1);
    let first = s.try_acquire(GB);
    assert!(first.is_some());
    assert!(s.try_acquire(GB).is_none(), "a second VM got the only slot");
    drop(first);
    assert!(s.try_acquire(GB).is_some());
}

/// Free slots are not enough: a VM starts only when its memory fits beside the others', so
/// twenty launches never ask macOS for more memory than the Mac has.
#[tokio::test]
async fn a_vm_waits_until_its_memory_fits() {
    let s = Scheduler::new(20);
    let mut big: Vec<_> = futures::future::join_all((0..3).map(|_| s.acquire(16 * GB, BUDGET))).await;
    assert_eq!(s.reserved_mb(), 48 * GB);
    let waiting = s.acquire(16 * GB, BUDGET);
    tokio::pin!(waiting);
    assert!(tokio::time::timeout(Duration::from_millis(100), waiting.as_mut()).await.is_err(), "64 GB > 56 GB");
    let small = tokio::time::timeout(Duration::from_millis(100), s.acquire(8 * GB, BUDGET)).await;
    drop(small);
    drop(big.pop());
    let fourth = tokio::time::timeout(Duration::from_secs(1), waiting).await.expect("starts once memory is freed");
    assert_eq!(s.reserved_mb(), 48 * GB);
    drop(fourth);
}

/// One VM larger than the budget still runs when it is alone: the queue never stalls.
#[tokio::test]
async fn a_lone_vm_always_starts() {
    let s = Scheduler::new(4);
    let only = tokio::time::timeout(Duration::from_millis(100), s.acquire(64 * GB, BUDGET)).await;
    assert!(only.is_ok());
}

/// A VM already running after a restart counts at once, even beyond the budget.
#[tokio::test]
async fn running_vms_count_against_the_memory_of_queued_ones() {
    let s = Scheduler::new(8);
    let _running = s.try_acquire(50 * GB).unwrap();
    assert!(tokio::time::timeout(Duration::from_millis(100), s.acquire(8 * GB, BUDGET)).await.is_err());
}

/// VMs found running after a restart all count in memory, even beyond the slots (the limit was
/// lowered before the restart): queued VMs must not start on top of them and push the Mac into swap.
#[tokio::test]
async fn running_vms_found_after_a_restart_all_count_in_memory() {
    let s = agentvm::app::scheduler::Scheduler::new(1);
    let _a = s.reattach(4096);
    let _b = s.reattach(4096);
    assert_eq!(s.reserved_mb(), 8192);
}

/// Memory this Mac really has free counts too: another agentvm, tests or apps may be using it.
/// A VM that does not fit waits, and starts once the memory is back (checked every 2 s).
#[tokio::test]
async fn a_vm_waits_for_memory_the_mac_really_has_free() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    let s = agentvm::app::scheduler::Scheduler::new(4);
    let free = Arc::new(AtomicU64::new(1_000));
    let seen = free.clone();
    let host = move || Some(seen.load(Ordering::Relaxed));
    let waiting =
        tokio::time::timeout(std::time::Duration::from_millis(500), s.acquire_with(4_096, 60_000, &host)).await;
    assert!(waiting.is_err(), "started without the memory");
    free.store(8_000, Ordering::Relaxed);
    let started = tokio::time::timeout(std::time::Duration::from_secs(5), s.acquire_with(4_096, 60_000, &host)).await;
    assert!(started.is_ok(), "never started once the memory was free");
}
