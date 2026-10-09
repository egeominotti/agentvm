//! What a task leaves on disk survives concurrent updates: the last state written is the last
//! state reached, and the file is always whole.

use std::sync::Arc;

use agentvm::app::store::Store;
use agentvm::domain::outcome::Final;
use agentvm::domain::task::{TaskEvent, TaskState};
use agentvm::domain::usage::AgentUsage;

use crate::helpers::{git_repo, running_task};

/// Cost updates arriving while the VM finishes must not write an older copy of the task over
/// the finished one: after a restart it would look running again.
#[test]
fn usage_updates_never_overwrite_a_newer_state_on_disk() {
    for _ in 0..40 {
        let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
        let id = running_task(home.path(), &repo, false);
        let jobs = home.path().join("jobs");
        let store = Arc::new(Store::persistent(jobs.clone()));
        store.insert(Store::load(&jobs).remove(0));
        store.apply(&id, TaskEvent::VmExited).unwrap();
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writers: Vec<_> = (0..6)
            .map(|w| {
                let (store, id, done) = (store.clone(), id.clone(), done.clone());
                std::thread::spawn(move || {
                    let mut i = 0;
                    // Up to the moment the task finishes, as the cost follower does.
                    while !done.load(std::sync::atomic::Ordering::Relaxed) {
                        i += 1;
                        store.set_usage(&id, AgentUsage { cost_usd: f64::from(w * 100_000 + i), ..Default::default() });
                    }
                })
            })
            .collect();
        std::thread::sleep(std::time::Duration::from_millis(2));
        store.apply(&id, TaskEvent::Finished(Final::NoChanges, id.branch())).unwrap();
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        for w in writers {
            w.join().unwrap();
        }
        let (loaded, unreadable) = Store::load_all(&jobs);
        assert!(unreadable.is_empty(), "a torn record.json");
        assert_eq!(loaded[0].state, TaskState::NoChanges, "the finished task went back to {:?}", loaded[0].state);
        let leftovers: Vec<_> = std::fs::read_dir(jobs.join(id.as_str()))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}

/// A record that cannot be written (the disk is full, the folder is in the way) is not dropped:
/// it is written as soon as it can be, so a restart never sees an older state.
#[test]
fn a_record_that_could_not_be_written_is_written_later() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let jobs = home.path().join("jobs");
    let store = Store::persistent(jobs.clone());
    let record = crate::helpers::record(&repo);
    let id = record.id.clone();
    store.insert(record);
    let file = jobs.join(id.as_str()).join("record.json");
    // A directory where the file goes: every write fails until it is gone.
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    store.apply(&id, TaskEvent::SlotAcquired).unwrap();
    std::fs::remove_dir(&file).unwrap();
    store.retry_unsaved();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(saved["state"]["state"], "preparing", "{saved}");
}
