//! Who may run a VM now: a free slot (resizable at runtime) and room in this Mac's memory.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

pub struct Scheduler {
    slots: Arc<Semaphore>,
    concurrency: AtomicUsize,
    memory: Arc<Memory>,
}

/// Memory reserved by the VMs holding a slot, and a signal when some is given back.
#[derive(Default)]
struct Memory {
    reserved_mb: Mutex<u64>,
    freed: Notify,
}

/// A running VM's place: its slot and its memory, both given back when dropped.
pub struct Slot {
    _permit: OwnedSemaphorePermit,
    mb: u64,
    memory: Arc<Memory>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        *self.memory.reserved_mb.lock().unwrap() -= self.mb;
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
        let permit = self.slots.clone().acquire_owned().await.expect("semaphore is never closed");
        loop {
            // Registered before looking, so memory freed in between is never missed.
            let freed = self.memory.freed.notified();
            tokio::pin!(freed);
            freed.as_mut().enable();
            {
                let mut reserved = self.memory.reserved_mb.lock().unwrap();
                if *reserved == 0 || *reserved + mb <= budget_mb {
                    *reserved += mb;
                    return Slot { _permit: permit, mb, memory: self.memory.clone() };
                }
            }
            freed.await;
        }
    }

    /// A slot right now for a VM that is already running (found after a restart): its memory
    /// counts at once, whatever the budget. `None` when all slots are taken.
    pub fn try_acquire(&self, mb: u64) -> Option<Slot> {
        let permit = self.slots.clone().try_acquire_owned().ok()?;
        *self.memory.reserved_mb.lock().unwrap() += mb;
        Some(Slot { _permit: permit, mb, memory: self.memory.clone() })
    }

    /// Memory reserved by the VMs that hold a slot.
    pub fn reserved_mb(&self) -> u64 {
        *self.memory.reserved_mb.lock().unwrap()
    }

    pub fn concurrency(&self) -> usize {
        self.concurrency.load(Ordering::SeqCst)
    }

    /// Growing frees slots at once; shrinking takes slots away as running VMs finish.
    pub fn resize(&self, n: usize) {
        let n = n.max(1);
        let current = self.concurrency.swap(n, Ordering::SeqCst);
        if n > current {
            self.slots.add_permits(n - current);
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
