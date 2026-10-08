//! Identifiers and validated inputs.

use std::time::{Duration, UNIX_EPOCH};

use agentvm::domain::ids::{CommitSha, Prompt, RepoPath, TaskId};

/// Every VM gets a UUIDv7 (RFC 9562): its creation time in ms, then 74 random bits.
#[test]
fn task_ids_are_uuid_v7() {
    // 2026-10-08 15:45:01.234 UTC = 0x01a1_1c30_7932 ms since the epoch.
    let now = UNIX_EPOCH + Duration::from_millis(1_791_474_301_234);
    let id = TaskId::generate(now, &[0xff; 10]);
    assert_eq!(id.as_str(), "01a11c30-7932-7fff-bfff-ffffffffffff");
    assert_eq!(id.branch(), format!("agent/{id}"));
    let zero = TaskId::generate(now, &[0; 10]);
    assert_eq!(&zero.as_str()[14..15], "7", "version");
    assert_eq!(&zero.as_str()[19..20], "8", "variant 10xx");
    assert_eq!(TaskId::parse(id.as_str()), Some(id.clone()));
}

/// Ordered by time like the ids before them, so lists and folders still read oldest to newest.
#[test]
fn uuid_v7_ids_sort_by_time() {
    let early = TaskId::generate(UNIX_EPOCH + Duration::from_millis(1_000), &[0xff; 10]);
    let late = TaskId::generate(UNIX_EPOCH + Duration::from_millis(1_001), &[0; 10]);
    assert!(early.as_str() < late.as_str());
}

/// Tasks, snapshots and branches made before UUIDs keep working.
#[test]
fn ids_from_before_uuids_are_still_read() {
    assert!(TaskId::parse("20261008-154501-a3f9").is_some());
    assert!(TaskId::parse("0199c4b6-a2f2-7fff-bfff-ffffffffffff").is_some());
    for bad in [
        "0199c4b6-a2f2-4fff-bfff-ffffffffffff", // version 4
        "0199C4B6-A2F2-7FFF-BFFF-FFFFFFFFFFFF", // upper case: one spelling per id
        "0199c4b6-a2f2-7fff-bfff-fffffffffff",
        "../../etc",
        "",
    ] {
        assert!(TaskId::parse(bad).is_none(), "{bad}");
    }
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
