//! Requests to a running VM through its shared folder: `<request>.request` holds a fresh id,
//! the guest's reply echoes it. One request at a time per VM, and a late reply to an earlier
//! request (one the host gave up on) is never taken for the current one.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::adapters::jobdir::{JobWorkspace, write_request};
use crate::domain::guest_reply::answer_to;
use crate::domain::ids::TaskId;
use crate::guestfs;

#[derive(Default)]
pub struct GuestChannel {
    /// Per VM: held for the whole of a request, so requests never overlap.
    busy: Mutex<HashMap<TaskId, Arc<tokio::sync::Mutex<()>>>>,
}

/// How a request ended.
#[derive(Debug)]
pub enum Answer {
    /// `name` is the reply file that answered, `bytes` its content.
    Reply { name: &'static str, bytes: Vec<u8> },
    /// The VM went away (or stopped running) before answering.
    Gone,
}

#[derive(Debug, thiserror::Error)]
pub enum AskError {
    #[error("the VM did not answer in time")]
    Timeout,
    #[error("could not reach the VM: {0}")]
    Io(#[from] std::io::Error),
}

impl GuestChannel {
    /// Asks VM `id` for `request` and waits for one of `replies` answering it, until `gone()`
    /// says the VM is no longer there, or `timeout`.
    pub async fn ask(
        &self,
        jobs: &Path,
        id: &TaskId,
        request: &str,
        replies: &[&'static str],
        timeout: Duration,
        gone: impl Fn() -> bool,
    ) -> Result<Answer, AskError> {
        let turn = self.turn(id);
        let _turn = turn.lock().await;
        let share = JobWorkspace::share_of(jobs, id);
        for name in replies {
            let _ = std::fs::remove_file(share.join(name));
        }
        let request_id = fresh_id();
        write_request(&share, &format!("{request}.request"), &request_id)?;
        let t0 = tokio::time::Instant::now();
        loop {
            for &name in replies {
                let path = share.join(name);
                if let Some(bytes) = guestfs::read(&path, 64 * 1024)
                    && answer_to(&bytes, &request_id).is_some()
                {
                    let _ = std::fs::remove_file(&path);
                    return Ok(Answer::Reply { name, bytes });
                }
            }
            if gone() {
                return Ok(Answer::Gone);
            }
            if t0.elapsed() > timeout {
                return Err(AskError::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    fn turn(&self, id: &TaskId) -> Arc<tokio::sync::Mutex<()>> {
        let mut busy = self.busy.lock().unwrap();
        // VMs nobody is asking anything any more: forget them.
        busy.retain(|_, turn| Arc::strong_count(turn) > 1);
        busy.entry(id.clone()).or_default().clone()
    }
}

/// Unique for this server's life: letters, digits and dashes only (the guest keeps just those).
fn fresh_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{}-{nanos}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))
}
