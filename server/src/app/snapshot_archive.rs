//! Snapshots as portable archives (a download, an S3 backup): the snapshot's files and the
//! compressed chunks of its disk, and back.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::backups::{ARCHIVE, BackupError, blocking, temp_file, unique};
use super::context::AppCtx;
use super::random::random_bytes;
use crate::adapters::archive;
use crate::adapters::snapshots::SnapshotStore;
use crate::domain::chunks::{ChunkId, Manifest};
use crate::domain::snapshot::{SnapshotId, SnapshotMeta};

/// Writes `<snapshot>.tar.zst` in a temporary folder and returns its path.
pub async fn export(ctx: &AppCtx, sid: &SnapshotId) -> Result<PathBuf, BackupError> {
    if ctx.snapshots.get(sid).is_none() {
        return Err(BackupError::NoSnapshot);
    }
    let file = temp_file(ctx, &format!("{sid}.{ARCHIVE}"))?;
    let out = file.path().to_path_buf();
    // The snapshot and the chunks of its disk, hard-linked into a folder of their own (instant,
    // nothing copied), then archived: the archive restores anywhere, chunks included.
    let staging = ctx.snapshots.scratch(&format!("export-{}", unique()))?;
    let (folder, manifest, chunks, dir) =
        (ctx.snapshots.folder(sid), ctx.snapshots.manifest(sid), ctx.chunks.clone(), staging.clone());
    let packed = blocking(move || {
        for entry in std::fs::read_dir(&folder)?.flatten() {
            std::fs::hard_link(entry.path(), dir.join(entry.file_name()))?;
        }
        if let Some(m) = manifest {
            std::fs::create_dir(dir.join("chunks"))?;
            for id in m.ids().collect::<std::collections::HashSet<_>>() {
                std::fs::hard_link(chunks.file(id), dir.join("chunks").join(format!("{}.zst", id.as_str())))?;
            }
        }
        Ok(archive::pack(&dir, &out)?)
    })
    .await;
    let _ = std::fs::remove_dir_all(&staging);
    packed?;
    // The caller owns the archive from here (and deletes it).
    let path = file.path().to_path_buf();
    std::mem::forget(file);
    Ok(path)
}

/// Adds an archive as a local snapshot. Keeps its id unless that id already exists here.
pub async fn import(ctx: &AppCtx, file: &Path) -> Result<SnapshotMeta, BackupError> {
    ctx.ensure_disk_space()?;
    let scratch = ctx.snapshots.scratch(&format!("import-{}", unique()))?;
    let adopted = adopt(ctx, file, &scratch).await;
    // Whatever happened, the scratch folder (possibly gigabytes) does not wait for a restart.
    let _ = std::fs::remove_dir_all(&scratch);
    adopted
}

async fn adopt(ctx: &AppCtx, file: &Path, scratch: &Path) -> Result<SnapshotMeta, BackupError> {
    let (from, to) = (file.to_path_buf(), scratch.to_path_buf());
    let meta = blocking(move || {
        archive::unpack(&from, &to)?;
        SnapshotStore::check_extracted(&to).map_err(|e| BackupError::Invalid(e.to_string()))?;
        // Bounded, never through a link: a snapshot's meta.json is a few hundred bytes.
        let bytes = crate::guestfs::read(&to.join("meta.json"), 64 << 10)
            .ok_or_else(|| BackupError::Invalid("the archive is not an agentvm snapshot".into()))?;
        serde_json::from_slice::<SnapshotMeta>(&bytes)
            .map_err(|_| BackupError::Invalid("the archive is not an agentvm snapshot".into()))
    })
    .await?;
    let id = match SnapshotId::parse(meta.id.as_str()) {
        Some(id) if ctx.snapshots.get(&id).is_none() => id,
        _ => SnapshotId::generate(SystemTime::now(), &random_bytes()),
    };
    // Its chunks join the store and its manifest is adopted under one lock: a cleanup between the
    // two would take them for unused.
    let _lock = ctx.chunk_lock.lock().await;
    let (chunks, dir) = (ctx.chunks.clone(), scratch.to_path_buf());
    blocking(move || adopt_chunks(&chunks, &dir)).await?;
    // Brought back by hand: kept until deleted by hand, never pruned as an automatic snapshot.
    let adopted = ctx.snapshots.adopt(scratch, SnapshotMeta { id, auto: false, compacting: false, ..meta })?;
    // An archive of a raw disk (from before chunks) is compacted like a new snapshot.
    ctx.compactor.notify_one();
    Ok(adopted)
}

/// Moves an archive's chunks into the store; its manifest must find every chunk it lists.
fn adopt_chunks(chunks: &crate::adapters::chunks::ChunkStore, dir: &Path) -> Result<(), BackupError> {
    let invalid = |why: &str| BackupError::Invalid(format!("the archive is not a complete agentvm snapshot: {why}"));
    let from = dir.join("chunks");
    if from.is_dir() {
        for entry in std::fs::read_dir(&from)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix(".zst").and_then(ChunkId::parse).ok_or_else(|| invalid("a stray file"))?;
            chunks.adopt(&id, &entry.path())?;
        }
        std::fs::remove_dir_all(&from)?;
    }
    if let Some(bytes) = crate::guestfs::read(&dir.join("disk.manifest.json"), 64 << 20) {
        let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| invalid("a damaged manifest"))?;
        if manifest.ids().any(|id| !chunks.file(id).is_file()) {
            return Err(invalid("chunks are missing"));
        }
    }
    Ok(())
}
