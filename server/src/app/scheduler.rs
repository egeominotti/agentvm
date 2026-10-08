//! Limit on concurrent VMs, resizable at runtime.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct Scheduler {
    slots: Arc<Semaphore>,
    concurrency: AtomicUsize,
}

impl Scheduler {
    pub fn new(concurrency: usize) -> Self {
        let n = concurrency.max(1);
        Scheduler { slots: Arc::new(Semaphore::new(n)), concurrency: AtomicUsize::new(n) }
    }

    pub async fn acquire(&self) -> OwnedSemaphorePermit {
        self.slots.clone().acquire_owned().await.expect("semaphore is never closed")
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
