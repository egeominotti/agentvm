//! Snapshot exports, imports and the temporary files they leave.

use std::time::SystemTime;

use crate::helpers::ctx;

/// A snapshot folder as `take_snapshot` leaves it, with a small disk.
fn stored_snapshot(home: &std::path::Path) -> agentvm::domain::snapshot::SnapshotId {
    stored_snapshot_of(home, false)
}

fn stored_snapshot_of(home: &std::path::Path, auto: bool) -> agentvm::domain::snapshot::SnapshotId {
    use agentvm::domain::snapshot::{SnapshotId, SnapshotMeta};
    let id = SnapshotId::generate(SystemTime::now(), &[1, 2]);
    let dir = home.join("snapshots").join(id.as_str());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("disk.raw"), vec![7u8; 4 << 20]).unwrap();
    std::fs::write(dir.join("efivars"), b"efi").unwrap();
    let meta = SnapshotMeta {
        id: id.clone(),
        name: "s".into(),
        source_task: "t".into(),
        repo: "/r".into(),
        base_sha: "a".repeat(40),
        model: "default".into(),
        claude_version: None,
        created_at: 1.0,
        size_mb: 4,
        cpus: 0,
        memory_mb: 0,
        auto,
    };
    std::fs::write(dir.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
    id
}

/// A download and an S3 backup of the same snapshot at once must not share a temporary file.
#[tokio::test]
async fn simultaneous_exports_of_a_snapshot_do_not_corrupt_each_other() {
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    let id = stored_snapshot(home.path());
    let (a, b) = tokio::join!(agentvm::app::backups::export(&ctx, &id), agentvm::app::backups::export(&ctx, &id));
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a, b, "both exports wrote the same file");
    for archive in [&a, &b] {
        let out = tempfile::tempdir().unwrap();
        agentvm::adapters::archive::unpack(archive, out.path()).unwrap();
        assert_eq!(std::fs::read(out.path().join("disk.raw")).unwrap().len(), 4 << 20);
    }
}

/// Imports at the same time each get their own scratch folder.
#[tokio::test]
async fn simultaneous_imports_each_produce_a_complete_snapshot() {
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    let id = stored_snapshot(home.path());
    let file = agentvm::app::backups::export(&ctx, &id).await.unwrap();
    let copy = file.with_extension("copy.zst");
    std::fs::copy(&file, &copy).unwrap();
    let (x, y) = tokio::join!(agentvm::app::backups::import(&ctx, &file), agentvm::app::backups::import(&ctx, &copy));
    let (x, y) = (x.unwrap(), y.unwrap());
    assert_ne!(x.id, y.id);
    for m in [x, y] {
        let dir = home.path().join("snapshots").join(m.id.as_str());
        assert_eq!(std::fs::read(dir.join("disk.raw")).unwrap().len(), 4 << 20);
    }
}

/// A backup brought back (from S3 or a file) is kept until deleted by hand: the next automatic
/// snapshot of the machine it came from must not prune it as one of its own.
#[tokio::test]
async fn an_imported_backup_is_never_pruned_as_an_automatic_snapshot() {
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    let id = stored_snapshot_of(home.path(), true);
    let file = agentvm::app::backups::export(&ctx, &id).await.unwrap();
    std::fs::remove_dir_all(home.path().join("snapshots").join(id.as_str())).unwrap();
    let back = agentvm::app::backups::import(&ctx, &file).await.unwrap();
    assert!(!back.auto, "{back:?}");
    assert!(!ctx.snapshots.get(&back.id).unwrap().auto);
}

/// Temporary files left by a crash or a failed transfer are removed when the server starts.
#[test]
fn leftover_temporary_files_are_cleaned_up_at_start() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("tmp")).unwrap();
    std::fs::write(home.path().join("tmp/upload-ab12.tar.zst"), b"partial").unwrap();
    std::fs::create_dir_all(home.path().join("snapshots/.import-77")).unwrap();
    // A repository bundle whose packing a crash cut short.
    std::fs::create_dir_all(home.path().join("cache/bundles")).unwrap();
    std::fs::write(home.path().join("cache/bundles/abc-def.123-0.tmp"), b"half a bundle").unwrap();
    std::fs::write(home.path().join("cache/bundles/abc-def.bundle"), b"a whole one").unwrap();
    let id = stored_snapshot(home.path());
    agentvm::app::backups::remove_leftovers(&ctx(home.path()));
    assert!(!home.path().join("tmp/upload-ab12.tar.zst").exists());
    assert!(!home.path().join("snapshots/.import-77").exists());
    assert!(!home.path().join("cache/bundles/abc-def.123-0.tmp").exists());
    assert!(home.path().join("cache/bundles/abc-def.bundle").exists(), "a finished bundle was removed");
    assert!(home.path().join("snapshots").join(id.as_str()).join("disk.raw").exists(), "a real snapshot was removed");
}

/// An archive is data from elsewhere: links and special files in it are refused, so it can never
/// make a host file the VM's disk, hang the server on a FIFO, or fill its memory from /dev/zero.
#[tokio::test(flavor = "multi_thread")]
async fn an_archive_with_links_or_special_files_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    let id = stored_snapshot(home.path());
    let src = home.path().join("snapshots").join(id.as_str());
    for (file, target) in [("disk.raw", "/etc/hosts"), ("meta.json", "/dev/zero")] {
        let crafted = tempfile::tempdir().unwrap();
        for f in ["disk.raw", "efivars", "meta.json"] {
            std::fs::copy(src.join(f), crafted.path().join(f)).unwrap();
        }
        std::fs::remove_file(crafted.path().join(file)).unwrap();
        std::os::unix::fs::symlink(target, crafted.path().join(file)).unwrap();
        let archive = home.path().join(format!("crafted-{file}.tar.zst"));
        agentvm::adapters::archive::pack(crafted.path(), &archive).unwrap();
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(20), agentvm::app::backups::import(&ctx, &archive))
                .await
                .expect("the import hung");
        assert!(result.is_err(), "an archive with {file} -> {target} was imported");
    }
    assert_eq!(ctx.snapshots.list().len(), 1, "only the snapshot made here");
}

/// A machine waiting in the queue to start from a snapshot needs it: deleting it is refused.
#[test]
fn a_snapshot_a_queued_machine_starts_from_cannot_be_deleted() {
    let (home, repo) = (tempfile::tempdir().unwrap(), crate::helpers::git_repo());
    let ctx = ctx(home.path());
    let sid = stored_snapshot(home.path());
    let mut waiting = crate::helpers::record(&repo);
    waiting.restore_from = Some(sid.clone());
    ctx.store.insert(waiting);
    let err = agentvm::app::snapshots::delete(&ctx, &sid).unwrap_err();
    assert!(err.to_string().contains("waiting"), "{err}");
    assert!(ctx.snapshots.get(&sid).is_some());
}
