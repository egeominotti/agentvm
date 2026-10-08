//! A failed VM's diagnostics: why, what to do, when, and the evidence.

use agentvm::app::diagnostics::of;
use agentvm::domain::task::TaskEvent;

use crate::helpers::{ctx, git_repo, running_task};

#[test]
fn a_failed_vm_explains_itself_from_its_files() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let id = running_task(home.path(), &repo, false);
    let job = home.path().join("jobs").join(id.as_str());
    let share = job.join("share");
    std::fs::create_dir_all(&share).unwrap();
    std::fs::write(
        share.join("job.log"),
        "[2.0s] job start\n[3.1s] running .agentvm/setup.sh\n[9.2s] setup failed (see setup.log)\n",
    )
    .unwrap();
    std::fs::write(share.join("setup.log"), "npm ERR! missing script: bootstrap\n").unwrap();
    std::fs::write(job.join("console.log"), "Booting Debian\n").unwrap();
    // The guest controls share/: a symlink it planted must never make the host read a Mac file.
    std::os::unix::fs::symlink("/etc/hosts", share.join("claude.err")).unwrap();
    let ctx = ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    ctx.store.apply(&id, TaskEvent::VmExited).unwrap();
    ctx.store.apply(&id, TaskEvent::Failure("claude_exit 1".into())).unwrap();

    let d = of(&ctx, &id).unwrap();
    assert_eq!(d.summary, "claude_exit 1");
    assert!(d.hint.as_deref().unwrap_or_default().contains("setup.sh"), "{:?}", d.hint);
    assert_eq!(d.timeline.last().map(|s| s.state.as_str()), Some("failed"));
    let names: Vec<_> = d.logs.iter().map(|l| l.file.as_str()).collect();
    assert_eq!(names, ["share/job.log", "share/setup.log", "console.log"], "{names:?}");
    assert!(d.logs[1].tail.contains("missing script"));
    assert!(!d.logs.iter().any(|l| l.tail.contains("localhost")), "read a Mac file through a symlink");
}

/// "Delete logs of closed VMs" removed the folder: the summary and the timeline remain.
#[test]
fn a_vm_whose_folder_is_gone_still_has_a_summary_and_a_timeline() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let id = running_task(home.path(), &repo, false);
    let ctx = ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    ctx.store.apply(&id, TaskEvent::Failure("timeout".into())).unwrap();
    std::fs::remove_dir_all(home.path().join("jobs").join(id.as_str())).unwrap();
    let d = of(&ctx, &id).unwrap();
    assert!(d.hint.unwrap().contains("time limit"));
    assert!(d.logs.is_empty());
    assert!(!d.timeline.is_empty());
}
