//! A Mac running out of disk: launches, snapshots and imports are refused with a clear message
//! up front, instead of VMs failing halfway through their writes.

use agentvm::app::submission::{NewTask, SubmitError, submit};

use crate::helpers::{ctx_with, git_repo, running_task};

/// Asks for more free space than any disk has: a real check on the real disk, no fake.
const ALL_OF_IT: u64 = u64::MAX / 4;

fn new_task(repo: &str) -> NewTask<'_> {
    NewTask {
        repo,
        prompt: "x".into(),
        base_ref: None,
        interactive: false,
        model: None,
        claude_version: None,
        restore_from: None,
        cpus: Some(1),
        memory_mb: Some(1024),
        label: None,
        tailscale: None,
    }
}

#[tokio::test]
async fn a_launch_is_refused_when_the_disk_is_nearly_full() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let ctx = ctx_with(home.path(), ALL_OF_IT);
    let err = submit(&ctx, new_task(&repo.path().display().to_string())).unwrap_err();
    assert!(matches!(err, SubmitError::DiskFull(_)), "{err}");
    assert!(err.to_string().contains("free"), "{err}");
    assert!(ctx.store.list().is_empty());
}

#[tokio::test]
async fn a_snapshot_is_refused_when_the_disk_is_nearly_full() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let id = running_task(home.path(), &repo, true);
    let ctx = ctx_with(home.path(), ALL_OF_IT);
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    let err = agentvm::app::snapshots::take_snapshot(&ctx, &id, None).await.unwrap_err();
    assert!(matches!(err, agentvm::app::snapshots::SnapshotError::DiskFull(_)), "{err}");
}
