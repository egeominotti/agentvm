//! Who may run a VM now: a free slot (resizable at runtime) and room in this Mac's memory.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

pub struct Scheduler {
    slots: Arc<Semaphore>,
    concurrency: AtomicUsize,
    memory: Arc<Memory>,
}

/// Every active VM's memory and place in the concurrency limit, including recovered VMs.
#[derive(Default)]
struct Memory {
    allocation: Mutex<Allocation>,
    freed: Notify,
}

#[derive(Default)]
struct Allocation {
    reserved_mb: u64,
    active: usize,
}

/// A running VM's place: its slot and its memory, both given back when dropped.
pub struct Slot {
    /// `None` for a recovered VM beyond the limit: it still counts in memory and concurrency.
    _permit: Option<OwnedSemaphorePermit>,
    mb: u64,
    memory: Arc<Memory>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut allocation = self.memory.allocation.lock().unwrap();
        allocation.reserved_mb -= self.mb;
        allocation.active -= 1;
        drop(allocation);
        self.memory.freed.notify_waiters();
    }
}

impl Scheduler {
    pub fn new(concurrency: usize) -> Self {
        let n = concurrency.max(1);
        Scheduler { slots: Arc::new(Semaphore::new(n)), concurrency: AtomicUsize::new(n), memory: Arc::default() }
    }

    /// Waits for a slot, then until `mb` fits beside the memory already reserved within
    /// `budget_mb` (this Mac's memory minus what macOS keeps). A VM alone always starts.
    pub async fn acquire(&self, mb: u64, budget_mb: u64) -> Slot {
        self.acquire_with(mb, budget_mb, &|| None).await
    }

    /// [`Scheduler::acquire`], and also until the Mac really has `mb` free (`host_free_mb`, `None`
    /// when unknown): another agentvm, tests or apps may use memory this one never reserved.
    /// That memory is freed without telling us, so it is looked at again every 2 s.
    pub async fn acquire_with(&self, mb: u64, budget_mb: u64, host_free_mb: &(dyn Fn() -> Option<u64> + Sync)) -> Slot {
        let permit = self.slots.clone().acquire_owned().await.expect("semaphore is never closed");
        loop {
            // Registered before looking, so memory freed in between is never missed.
            let freed = self.memory.freed.notified();
            tokio::pin!(freed);
            freed.as_mut().enable();
            {
                let mut allocation = self.memory.allocation.lock().unwrap();
                let fits_slot = allocation.active < self.concurrency();
                let fits_here = allocation.active == 0
                    || allocation.reserved_mb.checked_add(mb).is_some_and(|total| total <= budget_mb);
                let fits_mac = host_free_mb().is_none_or(|free| free >= mb);
                if fits_slot && fits_here && fits_mac {
                    allocation.reserved_mb += mb;
                    allocation.active += 1;
                    return Slot { _permit: Some(permit), mb, memory: self.memory.clone() };
                }
            }
            tokio::select! {
                _ = freed => {}
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
            }
        }
    }

    /// A slot right now for a VM that is already running (found after a restart): its memory
    /// counts at once, whatever the budget. `None` when all slots are taken.
    pub fn try_acquire(&self, mb: u64) -> Option<Slot> {
        let mut allocation = self.memory.allocation.lock().unwrap();
        if allocation.active >= self.concurrency() {
            return None;
        }
        let permit = self.slots.clone().try_acquire_owned().ok()?;
        allocation.reserved_mb += mb;
        allocation.active += 1;
        Some(Slot { _permit: Some(permit), mb, memory: self.memory.clone() })
    }

    /// A VM already running when the server starts: a slot if one is left, and its memory counted
    /// in any case, so queued VMs never start on top of it beyond this Mac's memory.
    pub fn reattach(&self, mb: u64) -> Slot {
        self.try_acquire(mb).unwrap_or_else(|| {
            let mut allocation = self.memory.allocation.lock().unwrap();
            allocation.reserved_mb += mb;
            allocation.active += 1;
            Slot { _permit: None, mb, memory: self.memory.clone() }
        })
    }

    /// Memory reserved by the VMs that hold a slot.
    pub fn reserved_mb(&self) -> u64 {
        self.memory.allocation.lock().unwrap().reserved_mb
    }

    pub fn concurrency(&self) -> usize {
        self.concurrency.load(Ordering::SeqCst)
    }

    /// Growing frees slots at once; shrinking takes slots away as running VMs finish.
    pub fn resize(&self, n: usize) {
        let n = n.max(1);
        let allocation = self.memory.allocation.lock().unwrap();
        let current = self.concurrency.swap(n, Ordering::SeqCst);
        drop(allocation);
        if n > current {
            self.slots.add_permits(n - current);
            self.memory.freed.notify_waiters();
        } else if n < current {
            let slots = self.slots.clone();
            tokio::spawn(async move {
                if let Ok(permits) = slots.acquire_many_owned((current - n) as u32).await {
                    permits.forget();
                }
            });
        }
    }
}
