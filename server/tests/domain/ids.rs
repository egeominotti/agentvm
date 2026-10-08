//! Identifiers and validated inputs.

use std::time::{Duration, UNIX_EPOCH};

use agentvm::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};

#[test]
fn task_id_has_timestamp_and_random_suffix() {
    // 2026-10-08 15:45:01 UTC
    let now = UNIX_EPOCH + Duration::from_secs(1_791_474_301);
    let id = TaskId::generate(now, [0xa3, 0xf9]);
    assert_eq!(id.as_str(), "20261008-154501-a3f9");
    assert_eq!(id.branch(), "agent/20261008-154501-a3f9");
}

#[test]
fn commit_sha_requires_40_hex() {
    assert!(CommitSha::parse(&"a".repeat(40)).is_ok());
    assert!(CommitSha::parse("abc").is_err());
    assert!(CommitSha::parse(&"g".repeat(40)).is_err());
}

#[test]
fn prompt_rejects_blank() {
    assert!(Prompt::new("   \n".into()).is_err());
    assert_eq!(Prompt::new("  fix it ".into()).unwrap().as_str(), "fix it");
}

#[test]
fn repo_path_requires_git_dir() {
    let dir = tempfile::tempdir().unwrap();
    assert!(RepoPath::new(dir.path().to_path_buf()).is_err());
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    assert!(RepoPath::new(dir.path().to_path_buf()).is_ok());
}
