//! Limite di VM contemporanee.

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct Scheduler {
    slots: Arc<Semaphore>,
    concurrency: usize,
}

impl Scheduler {
    pub fn new(concurrency: usize) -> Self {
        Scheduler { slots: Arc::new(Semaphore::new(concurrency)), concurrency }
    }

    pub async fn acquire(&self) -> OwnedSemaphorePermit {
        self.slots.clone().acquire_owned().await.expect("semaforo mai chiuso")
    }

    pub fn concurrency(&self) -> usize {
        self.concurrency
    }

    pub fn running(&self) -> usize {
        self.concurrency - self.slots.available_permits()
    }
}
