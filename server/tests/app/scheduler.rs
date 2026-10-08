//! VM slots: resized at runtime, taken first by VMs found after a restart.

use std::time::Duration;

#[tokio::test]
async fn scheduler_grows_and_shrinks_at_runtime() {
    use agentvm::app::scheduler::Scheduler;
    let s = Scheduler::new(1);
    let a = s.acquire().await;
    assert!(tokio::time::timeout(Duration::from_millis(50), s.acquire()).await.is_err(), "only 1 slot");
    s.resize(2);
    let b = tokio::time::timeout(Duration::from_millis(200), s.acquire()).await.expect("second slot after grow");
    assert_eq!(s.concurrency(), 2);
    s.resize(1);
    drop(a);
    drop(b);
    let _c = s.acquire().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(tokio::time::timeout(Duration::from_millis(100), s.acquire()).await.is_err(), "back to 1 slot");
}

/// VMs found running after a restart take their slots before queued tasks may start.
#[test]
fn running_vms_take_their_slots_without_waiting() {
    let s = agentvm::app::scheduler::Scheduler::new(1);
    let first = s.try_acquire();
    assert!(first.is_some());
    assert!(s.try_acquire().is_none(), "a second VM got the only slot");
    drop(first);
    assert!(s.try_acquire().is_some());
}
