//! "Delete logs of closed VMs": only the folders of tasks known to be finished go.

use agentvm::app::queries::cleanup_finished_jobs;
use agentvm::domain::outcome::Final;
use agentvm::domain::task::TaskEvent;

use crate::helpers::{ctx, git_repo, running_task};

#[tokio::test]
async fn cleanup_never_touches_a_folder_it_cannot_account_for() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let jobs = home.path().join("jobs");
    let (finished, running) = (running_task(home.path(), &repo, false), running_task(home.path(), &repo, true));
    // A folder whose record this version cannot read: it may belong to a VM still running.
    let unknown = jobs.join("20261008-000000-ffff");
    std::fs::create_dir_all(&unknown).unwrap();
    std::fs::write(unknown.join("record.json"), "{ from a newer version").unwrap();
    std::fs::write(unknown.join("disk.raw"), b"work").unwrap();
    let ctx = ctx(home.path());
    for r in agentvm::app::store::Store::load(&jobs) {
        ctx.store.insert(r);
    }
    ctx.store.apply(&finished, TaskEvent::VmExited).unwrap();
    ctx.store.apply(&finished, TaskEvent::Finished(Final::NoChanges, finished.branch())).unwrap();

    assert_eq!(cleanup_finished_jobs(&ctx), 1);
    assert!(!jobs.join(finished.as_str()).exists());
    assert!(jobs.join(running.as_str()).join("disk.raw").exists(), "a running VM's folder");
    assert!(unknown.join("disk.raw").exists(), "a folder it cannot account for");
}
