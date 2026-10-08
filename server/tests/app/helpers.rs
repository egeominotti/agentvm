//! Task records, an application context on a temporary home, and a git repository.

use std::time::SystemTime;

use agentvm::app::store::TaskRecord;
use agentvm::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};

pub(crate) fn record(repo: &tempfile::TempDir) -> TaskRecord {
    std::fs::create_dir_all(repo.path().join(".git")).unwrap();
    TaskRecord::new(
        TaskId::generate(SystemTime::now(), [9, 9]),
        RepoPath::new(repo.path().to_path_buf()).unwrap(),
        Some(Prompt::new("do something".into()).unwrap()),
        CommitSha::parse(&"b".repeat(40)).unwrap(),
        false,
    )
}

pub(crate) fn ctx(home: &std::path::Path) -> std::sync::Arc<agentvm::app::supervisor::AppCtx> {
    let config = agentvm::config::Config {
        home: home.to_path_buf(),
        port: 7777,
        concurrency: 1,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 60,
        vm_helper: "agentvm-vm".into(),
        scripts_dir: "scripts".into(),
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
