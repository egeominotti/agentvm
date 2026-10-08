//! Closing a terminal: the dashboard learns whether the VM is going away or why it stays up.

use std::path::Path;
use std::time::Duration;

use agentvm::app::session::{SessionError, close};
use agentvm::domain::task::TaskEvent;

use crate::helpers::{ctx, git_repo, running_task};

/// The guest side of the shared folder: answers the flush before the snapshot, then `reply`
/// once the close request arrives.
fn guest(share: &Path, reply: impl FnOnce() + Send + 'static) {
    let share = share.to_path_buf();
    std::thread::spawn(move || {
        for _ in 0..600 {
            if share.join("sync.request").exists() {
                let _ = std::fs::remove_file(share.join("sync.request"));
                std::fs::write(share.join("sync.done"), "").unwrap();
            }
            if share.join("close.request").exists() {
                return reply();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
}

/// A final save that fails keeps the VM up (nothing is lost) and the dashboard is told why,
/// instead of showing "Closing…" forever.
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_final_save_is_reported_and_the_vm_keeps_running() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let id = running_task(home.path(), &repo, true);
    let ctx = ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    let share = home.path().join("jobs").join(id.as_str()).join("share");
    std::fs::create_dir_all(&share).unwrap();
    let reply = share.join("close.failed");
    guest(&share, move || std::fs::write(reply, r#"{"error":"git commit: index.lock exists"}"#).unwrap());
    let err = close(&ctx, &id).await.unwrap_err();
    assert!(matches!(&err, SessionError::CloseFailed(e) if e.contains("index.lock")), "{err}");
    assert!(!share.join("close.failed").exists(), "consumed: a later close must not read it again");
}

/// The usual case: the VM shuts down after its final save.
#[tokio::test(flavor = "multi_thread")]
async fn a_close_returns_once_the_vm_is_going_away() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let id = running_task(home.path(), &repo, true);
    let ctx = ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    let share = home.path().join("jobs").join(id.as_str()).join("share");
    std::fs::create_dir_all(&share).unwrap();
    let (store, gone) = (ctx.store.clone_handle(), id.clone());
    guest(&share, move || {
        store.apply(&gone, TaskEvent::VmExited).unwrap();
    });
    tokio::time::timeout(Duration::from_secs(20), close(&ctx, &id)).await.expect("close never returned").unwrap();
}
