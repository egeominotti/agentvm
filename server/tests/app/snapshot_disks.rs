//! Snapshot disks in the shared chunk store: compacted after they are taken, restored byte for
//! byte, their real cost shown, their chunks freed with them, and carried whole by an export.

use std::os::unix::fs::FileExt;
use std::path::Path;
use std::time::SystemTime;

use agentvm::app::snapshot_disks::{collect_garbage, compact, restore_disk};
use agentvm::domain::snapshot::{SnapshotId, SnapshotMeta};

use crate::helpers::ctx;

const MB: u64 = 1 << 20;

/// A VM disk: 32 MiB, mostly empty; `seed` changes one chunk (a file edited between snapshots).
fn disk(dir: &Path, seed: u8) -> std::path::PathBuf {
    let path = dir.join(format!("disk-{seed}.raw"));
    let f = std::fs::File::create(&path).unwrap();
    f.set_len(32 * MB).unwrap();
    let system: Vec<u8> = (0..4 * MB).map(|i| (i * 31 % 251) as u8).collect();
    f.write_all_at(&system, 0).unwrap();
    let edited: Vec<u8> = (0..MB).map(|i| ((i * 7 + u64::from(seed) * 13) % 241) as u8).collect();
    f.write_all_at(&edited, 20 * MB).unwrap();
    path
}

fn take(ctx: &agentvm::app::supervisor::AppCtx, disk: &Path, efivars: &Path) -> SnapshotId {
    let id = SnapshotId::generate(SystemTime::now(), &agentvm::app::random::random_bytes());
    let meta = SnapshotMeta {
        id: id.clone(),
        name: "s".into(),
        source_task: "t".into(),
        repo: "/r".into(),
        base_sha: "a".repeat(40),
        model: "default".into(),
        claude_version: None,
        created_at: 1.0,
        size_mb: 0,
        cpus: 0,
        memory_mb: 0,
        auto: false,
        compacting: false,
    };
    ctx.snapshots.create(&meta, disk, efivars).unwrap();
    id
}

#[tokio::test]
async fn a_snapshot_is_compacted_restored_exactly_and_freed_with_its_chunks() {
    let (home, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ctx = ctx(home.path());
    let efivars = work.path().join("efivars");
    std::fs::write(&efivars, b"efi").unwrap();
    let first = disk(work.path(), 1);
    let a = take(&ctx, &first, &efivars);
    assert!(agentvm::app::snapshots::list(&ctx)[0].compacting, "a raw clone first");

    compact(&ctx, &a).await.unwrap();
    assert!(!ctx.snapshots.has_raw_disk(&a), "the clone goes once compacted");
    let listed = agentvm::app::snapshots::list(&ctx);
    assert!(!listed[0].compacting);
    assert!(listed[0].size_mb <= 5, "5 MiB of data, compressed: {} MB", listed[0].size_mb);

    let back = work.path().join("restored.raw");
    restore_disk(&ctx, &a, back.clone()).await.unwrap();
    assert_eq!(std::fs::read(&back).unwrap(), std::fs::read(&first).unwrap());

    // A second snapshot of the same VM, one chunk edited: it costs that chunk only.
    let b = take(&ctx, &disk(work.path(), 2), &efivars);
    compact(&ctx, &b).await.unwrap();
    let chunks_before = ctx.chunks.present().unwrap().len();
    assert_eq!(chunks_before, 4 + 2, "the system's 4 chunks are stored once");

    agentvm::app::snapshots::delete(&ctx, &a).unwrap();
    collect_garbage(&ctx).await;
    assert_eq!(ctx.chunks.present().unwrap().len(), 4 + 1, "only a's own chunk went");
    let back = work.path().join("restored-b.raw");
    restore_disk(&ctx, &b, back.clone()).await.unwrap();
    assert_eq!(std::fs::read(&back).unwrap(), std::fs::read(work.path().join("disk-2.raw")).unwrap());
}

/// An export carries the chunks of its disk: imported on another Mac (another home), the
/// snapshot restores byte for byte.
#[tokio::test]
async fn an_exported_snapshot_restores_elsewhere() {
    let (home, other, work) =
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ctx = ctx(home.path());
    let efivars = work.path().join("efivars");
    std::fs::write(&efivars, b"efi").unwrap();
    let source = disk(work.path(), 3);
    let id = take(&ctx, &source, &efivars);
    compact(&ctx, &id).await.unwrap();
    let archive = agentvm::app::backups::export(&ctx, &id).await.unwrap();

    let there = crate::helpers::ctx(other.path());
    let imported = agentvm::app::backups::import(&there, &archive).await.unwrap();
    assert!(!there.snapshots.has_raw_disk(&imported.id));
    let back = work.path().join("there.raw");
    restore_disk(&there, &imported.id, back.clone()).await.unwrap();
    assert_eq!(std::fs::read(&back).unwrap(), std::fs::read(&source).unwrap());
    let _ = std::fs::remove_file(archive);
}
