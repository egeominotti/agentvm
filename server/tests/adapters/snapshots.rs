//! Snapshots on disk and the archives they travel in.

#[test]
fn snapshots_are_stored_listed_and_deleted() {
    use agentvm::adapters::snapshots::SnapshotStore;
    use agentvm::domain::snapshot::{SnapshotId, SnapshotMeta};
    let tmp = tempfile::tempdir().unwrap();
    let disk = tmp.path().join("disk.raw");
    let efi = tmp.path().join("efivars");
    std::fs::write(&disk, vec![3u8; 1 << 20]).unwrap();
    std::fs::write(&efi, b"efi").unwrap();
    let store = SnapshotStore::new(tmp.path().join("snapshots"));
    let id = SnapshotId::generate(std::time::SystemTime::now(), &[1, 2]);
    let meta = SnapshotMeta {
        id: id.clone(),
        name: "before refactor".into(),
        source_task: "20261008-120000-abcd".into(),
        repo: "/tmp/repo".into(),
        base_sha: "a".repeat(40),
        model: "default".into(),
        claude_version: None,
        created_at: 1.0,
        size_mb: 0,
        cpus: 2,
        memory_mb: 2048,
        auto: false,
    };
    store.create(&meta, &disk, &efi).unwrap();
    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "before refactor");
    assert_eq!(std::fs::read(store.disk(&id)).unwrap(), std::fs::read(&disk).unwrap());
    assert!(store.get(&id).is_some());
    store.delete(&id).unwrap();
    assert!(store.list().is_empty());
}

/// Two snapshots with the same id (same second, same random bits): the second is refused and
/// the first one stays as it was, never deleted by the failed attempt.
#[test]
fn a_snapshot_never_replaces_one_with_the_same_id() {
    use agentvm::adapters::snapshots::SnapshotStore;
    use agentvm::domain::snapshot::{SnapshotId, SnapshotMeta};
    let tmp = tempfile::tempdir().unwrap();
    let (disk, efi, other) = (tmp.path().join("disk.raw"), tmp.path().join("efivars"), tmp.path().join("other.raw"));
    std::fs::write(&disk, vec![3u8; 1 << 20]).unwrap();
    std::fs::write(&other, vec![9u8; 1 << 20]).unwrap();
    std::fs::write(&efi, b"efi").unwrap();
    let store = SnapshotStore::new(tmp.path().join("snapshots"));
    let id = SnapshotId::generate(std::time::SystemTime::now(), &[5, 5]);
    let meta = SnapshotMeta {
        id: id.clone(),
        name: "first".into(),
        source_task: "20261008-120000-abcd".into(),
        repo: "/tmp/repo".into(),
        base_sha: "a".repeat(40),
        model: "default".into(),
        claude_version: None,
        created_at: 1.0,
        size_mb: 0,
        cpus: 2,
        memory_mb: 2048,
        auto: false,
    };
    store.create(&meta, &disk, &efi).unwrap();
    let again = store.create(&SnapshotMeta { name: "second".into(), ..meta }, &other, &efi);
    assert_eq!(again.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(store.get(&id).unwrap().name, "first");
    assert_eq!(std::fs::read(store.disk(&id)).unwrap(), vec![3u8; 1 << 20]);
}

#[test]
fn snapshot_ids_reject_path_tricks() {
    use agentvm::domain::snapshot::SnapshotId;
    assert!(SnapshotId::parse("snap-20261008-120000-abcd").is_some());
    for bad in ["../etc", "snap-x", "20261008-120000-abcd", "snap-20261008-120000-abcd/.."] {
        assert!(SnapshotId::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn archives_pack_and_unpack_a_folder_exactly() {
    use agentvm::adapters::archive;
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir(&src).unwrap();
    std::fs::write(src.join("disk.raw"), vec![9u8; 3 << 20]).unwrap();
    std::fs::write(src.join("meta.json"), "{\"a\":1}").unwrap();
    let file = tmp.path().join("snap.tar.zst");
    archive::pack(&src, &file).unwrap();
    let out = tmp.path().join("out");
    archive::unpack(&file, &out).unwrap();
    assert_eq!(std::fs::read(out.join("disk.raw")).unwrap(), std::fs::read(src.join("disk.raw")).unwrap());
    assert_eq!(std::fs::read_to_string(out.join("meta.json")).unwrap(), "{\"a\":1}");
}
