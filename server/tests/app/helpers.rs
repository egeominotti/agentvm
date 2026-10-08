//! Task records, an application context on a temporary home, and a git repository.

use std::time::SystemTime;

use agentvm::app::store::{Store, TaskRecord};
use agentvm::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};
use agentvm::domain::task::TaskEvent;

/// Different random bits for every record of a test run: two tasks never share an id.
fn next_bits() -> [u8; 2] {
    static NEXT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed).to_be_bytes()
}

pub(crate) fn record(repo: &tempfile::TempDir) -> TaskRecord {
    std::fs::create_dir_all(repo.path().join(".git")).unwrap();
    TaskRecord::new(
        TaskId::generate(SystemTime::now(), next_bits()),
        RepoPath::new(repo.path().to_path_buf()).unwrap(),
        Some(Prompt::new("do something".into()).unwrap()),
        CommitSha::parse(&"b".repeat(40)).unwrap(),
        false,
    )
}

pub(crate) fn ctx(home: &std::path::Path) -> std::sync::Arc<agentvm::app::supervisor::AppCtx> {
    ctx_with(home, 0)
}

/// `min_free_mb`: the free disk space below which launches, snapshots and imports are refused.
pub(crate) fn ctx_with(home: &std::path::Path, min_free_mb: u64) -> std::sync::Arc<agentvm::app::supervisor::AppCtx> {
    let config = agentvm::config::Config {
        home: home.to_path_buf(),
        port: 7777,
        concurrency: 1,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 60,
        vm_helper: "agentvm-vm".into(),
        scripts_dir: "scripts".into(),
        min_free_mb,
    };
    let keychain = agentvm::adapters::keychain::Keychain::new(Some(home.join("none.keychain-db")));
    std::sync::Arc::new(agentvm::app::supervisor::AppCtx::new(config, keychain))
}

pub(crate) fn git_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        assert!(std::process::Command::new("git").arg("-C").arg(dir.path()).args(args).status().unwrap().success())
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "one"]);
    dir
}

/// A task in the Running state with a job folder and a small disk, as a VM leaves it.
pub(crate) fn running_task(home: &std::path::Path, repo: &tempfile::TempDir, interactive: bool) -> TaskId {
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
