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
        agent: Default::default(),
        model: "default".into(),
        claude_version: None,
        created_at: 1.0,
        size_mb: 4,
        cpus: 0,
        memory_mb: 0,
        auto,
        compacting: false,
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

/// A backup from elsewhere names a repository: it restores only into one on this Mac that already
/// has its exact commit (a clone of the same project), never into an unrelated folder or repo.
#[test]
fn a_backup_restores_only_into_a_clone_of_its_own_project() {
    let (home, other) = (tempfile::tempdir().unwrap(), crate::helpers::git_repo());
    let ctx = ctx(home.path());
    let sid = stored_snapshot(home.path());
    let err = agentvm::app::snapshots::restore(&ctx, &sid).unwrap_err().to_string();
    assert!(err.contains("/r"), "a missing repository was accepted: {err}");
    // A real repository, but not the snapshot's project: its commit is not there.
    let meta_file = home.path().join("snapshots").join(sid.as_str()).join("meta.json");
    let mut meta: serde_json::Value = serde_json::from_slice(&std::fs::read(&meta_file).unwrap()).unwrap();
    meta["repo"] = other.path().to_string_lossy().into();
    std::fs::write(&meta_file, meta.to_string()).unwrap();
    let err = agentvm::app::snapshots::restore(&ctx, &sid).unwrap_err().to_string();
    assert!(err.contains("aaaaaaa"), "restored into a repository without its commit: {err}");
    assert!(ctx.store.list().is_empty(), "a machine was started");
}

#[tokio::test]
async fn importing_a_damaged_chunk_never_poisons_the_shared_store() {
    let (home, other, crafted) =
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ctx = ctx(home.path());
    let id = stored_snapshot(home.path());
    agentvm::app::snapshot_disks::compact(&ctx, &id).await.unwrap();
    let archive = agentvm::app::backups::export(&ctx, &id).await.unwrap();
    agentvm::adapters::archive::unpack(&archive, crafted.path()).unwrap();
    let chunk = std::fs::read_dir(crafted.path().join("chunks")).unwrap().next().unwrap().unwrap().path();
    std::fs::write(&chunk, zstd::bulk::compress(&vec![8; 1 << 20], 1).unwrap()).unwrap();
    let damaged = home.path().join("damaged.tar.zst");
    agentvm::adapters::archive::pack(crafted.path(), &damaged).unwrap();

    let there = crate::helpers::ctx(other.path());
    assert!(agentvm::app::backups::import(&there, &damaged).await.is_err(), "a chunk whose hash is wrong was imported");
    assert!(there.snapshots.list().is_empty());
    assert!(there.chunks.present().unwrap().is_empty(), "the damaged chunk entered the shared store");
}

#[tokio::test]
async fn concurrent_imports_into_an_empty_home_each_get_their_own_snapshot() {
    let (home, other) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ctx = ctx(home.path());
    let id = stored_snapshot(home.path());
    let file = agentvm::app::backups::export(&ctx, &id).await.unwrap();
    let there = crate::helpers::ctx(other.path());
    // Hold the adoption lock until every importer has unpacked the same original id.
    let lock = there.chunk_lock.lock().await;
    let imports: Vec<_> = (0..4)
        .map(|_| {
            let (there, file) = (there.clone(), file.clone());
            tokio::spawn(async move { agentvm::app::backups::import(&there, &file).await })
        })
        .collect();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    drop(lock);
    let results = futures::future::join_all(imports).await;
    for result in results {
        assert!(result.unwrap().is_ok(), "concurrent imports selected the same snapshot id");
    }
    assert_eq!(there.snapshots.list().len(), 4);
}

#[tokio::test]
async fn an_export_waits_for_compaction_and_carries_the_complete_disk() {
    let (home, other) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ctx = ctx(home.path());
    let id = stored_snapshot(home.path());
    let original = std::fs::read(ctx.snapshots.disk(&id)).unwrap();
    let lock = ctx.chunk_lock.lock().await;
    let (export_ctx, export_id) = (ctx.clone(), id.clone());
    let mut exporting = tokio::spawn(async move { agentvm::app::backups::export(&export_ctx, &export_id).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut exporting).await.is_err(),
        "an export read the snapshot while its disk representation could change"
    );
    // The compactor's real operations, while holding the same lock it holds in production.
    let manifest = ctx.chunks.pack(&ctx.snapshots.disk(&id)).unwrap();
    ctx.snapshots.compacted(&id, &manifest).unwrap();
    drop(lock);
    let archive = exporting.await.unwrap().unwrap();
    let there = crate::helpers::ctx(other.path());
    let imported = agentvm::app::backups::import(&there, &archive).await.unwrap();
    assert!(!there.snapshots.has_raw_disk(&imported.id));
    let restored = other.path().join("restored.raw");
    agentvm::app::snapshot_disks::restore_disk(&there, &imported.id, restored.clone()).await.unwrap();
    assert!(std::fs::read(restored).unwrap() == original, "the exported disk lost data during compaction");
}

#[tokio::test]
#[ignore = "needs the dev S3 server: scripts/dev-s3.sh up"]
async fn simultaneous_restores_of_an_s3_backup_each_produce_a_complete_snapshot() {
    struct TempKeychain(std::path::PathBuf);
    impl Drop for TempKeychain {
        fn drop(&mut self) {
            let _ = std::process::Command::new("security").arg("delete-keychain").arg(&self.0).output();
        }
    }
    let home = tempfile::tempdir().unwrap();
    let keychain = TempKeychain(home.path().join("none.keychain-db"));
    assert!(
        std::process::Command::new("security")
            .args(["create-keychain", "-p", "x"])
            .arg(&keychain.0)
            .status()
            .unwrap()
            .success()
    );
    let ctx = ctx(home.path());
    let cfg = agentvm::domain::s3::S3Config {
        endpoint: "http://127.0.0.1:9100".into(),
        region: "us-east-1".into(),
        bucket: "agentvm-backups".into(),
        prefix: format!("restore-{}", home.path().file_name().unwrap().to_string_lossy()),
        access_key: "agentvm".into(),
        path_style: true,
    };
    ctx.keychain.write_s3_secret(&agentvm::secret::Secret::new("agentvm-local-secret".into())).unwrap();
    ctx.settings.set_s3(Some(cfg)).unwrap();
    let id = stored_snapshot(home.path());
    agentvm::app::backups::backup(&ctx, &id).await.unwrap();
    ctx.snapshots.delete(&id).unwrap();
    let results = futures::future::join_all((0..4).map(|_| agentvm::app::backups::restore(&ctx, &id))).await;
    agentvm::app::backups::delete(&ctx, &id).await.unwrap();
    assert!(results.iter().all(Result::is_ok), "{results:?}");
    assert_eq!(ctx.snapshots.list().len(), 4);
    for result in results {
        let snapshot = result.unwrap();
        assert_eq!(std::fs::read(ctx.snapshots.disk(&snapshot.id)).unwrap(), vec![7; 4 << 20]);
    }
    assert!(std::fs::read_dir(home.path().join("tmp")).unwrap().next().is_none(), "downloads left files behind");
}
