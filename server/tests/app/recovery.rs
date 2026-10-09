//! Recovery after a restart: tasks whose VM went away while the server was down.

use std::time::Duration;

use agentvm::domain::ids::TaskId;

use crate::helpers::{ctx, git_repo, record, running_task as interrupted_task};

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

/// A task still queued at start-up is launched again at once; the clean-up of folders nobody
/// owns runs right after and must leave its folder (and the disk it may be cloning) alone.
#[tokio::test(flavor = "multi_thread")]
async fn queued_tasks_own_their_folder_at_start_up() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let rec = record(&repo);
    let id = rec.id.clone();
    agentvm::app::store::Store::persistent(home.path().join("jobs")).insert(rec);
    let ctx = ctx(home.path());
    let live = agentvm::app::supervisor::recover(&ctx);
    assert!(live.contains(id.as_str()), "{live:?}");
}

/// Force stop powers the VM off at once, so it saves nothing: its disk is kept too (a Stop must
/// never be the way work gets lost, least of all for a VM that stopped answering).
#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_vm_keeps_its_disk_as_a_snapshot() {
    let home = tempfile::tempdir().unwrap();
    let repo = git_repo();
    let id = interrupted_task(home.path(), &repo, true);
    let file = home.path().join("jobs").join(id.as_str()).join("record.json");
    let mut rec: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    rec["stop_requested"] = true.into();
    std::fs::write(&file, rec.to_string()).unwrap();
    let ctx = ctx(home.path());
    agentvm::app::supervisor::recover(&ctx);
    let rec = wait_terminal(&ctx, &id).await;
    assert!(matches!(rec.state, agentvm::domain::task::TaskState::Stopped), "{:?}", rec.state);
    let snaps = ctx.snapshots.list();
    assert_eq!(snaps.len(), 1, "the stopped VM's disk was thrown away: {snaps:?}");
    assert!(snaps[0].name.starts_with("Interrupted"), "resumable like any interrupted disk: {}", snaps[0].name);
    assert_eq!(std::fs::read(ctx.snapshots.disk(&snaps[0].id)).unwrap(), vec![7u8; 4 << 20]);
}
