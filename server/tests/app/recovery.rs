//! Recovery after a restart: tasks whose VM went away while the server was down.

use std::time::Duration;

use agentvm::app::store::Store;
use agentvm::domain::ids::TaskId;
use agentvm::domain::task::TaskEvent;

use crate::helpers::{ctx, git_repo, record};

/// A task left Running by a server that went away (a reboot, a crash) whose VM is gone.
fn interrupted_task(home: &std::path::Path, repo: &tempfile::TempDir, interactive: bool) -> TaskId {
    let mut rec = record(repo);
    rec.interactive = interactive;
    let id = rec.id.clone();
    let store = Store::persistent(home.join("jobs"));
    store.insert(rec);
    for e in [TaskEvent::SlotAcquired, TaskEvent::Prepared, TaskEvent::VmStarted] {
        store.apply(&id, e).unwrap();
    }
    let job = home.join("jobs").join(id.as_str());
    std::fs::write(job.join("disk.raw"), vec![7u8; 4 << 20]).unwrap();
    std::fs::write(job.join("efivars"), b"efi").unwrap();
    id
}

async fn wait_terminal(ctx: &agentvm::app::supervisor::AppCtx, id: &TaskId) -> agentvm::app::store::TaskRecord {
    for _ in 0..100 {
        let r = ctx.store.get(id).unwrap();
        if r.state.is_terminal() {
            return r;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the task never finished: {:?}", ctx.store.get(id).map(|r| r.state));
}

/// A reboot or a crash must never throw away a VM's disk: it holds work nobody saved yet.
#[tokio::test(flavor = "multi_thread")]
async fn a_vm_gone_while_the_server_was_down_keeps_its_disk_as_a_snapshot() {
    for interactive in [true, false] {
        let home = tempfile::tempdir().unwrap();
        let repo = git_repo();
        let id = interrupted_task(home.path(), &repo, interactive);
        let ctx = ctx(home.path());
        agentvm::app::supervisor::recover(&ctx);
        let rec = wait_terminal(&ctx, &id).await;
        let reason = format!("{:?}", rec.state);
        assert!(reason.contains("Snapshots"), "the user is told where the work is: {reason}");
        let snaps = ctx.snapshots.list();
        assert_eq!(snaps.len(), 1, "{snaps:?}");
        assert_eq!(snaps[0].source_task, id.as_str());
        assert!(snaps[0].name.starts_with("Interrupted"), "{}", snaps[0].name);
        assert!(!snaps[0].auto, "never pruned");
        assert_eq!(std::fs::read(ctx.snapshots.disk(&snaps[0].id)).unwrap(), vec![7u8; 4 << 20]);
        assert!(!home.path().join("jobs").join(id.as_str()).join("disk.raw").exists());
    }
}
