//! Snapshot disks in the shared chunk store: a snapshot is taken as an instant clone, then
//! compacted in the background (its new chunks compressed, the clone deleted); restoring writes
//! the disk back from its chunks; chunks no snapshot uses any more are deleted.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::context::AppCtx;
use crate::domain::chunks::{own_bytes, unreferenced};
use crate::domain::snapshot::SnapshotId;

/// Off the async workers: packing reads gigabytes and keeps every core busy.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> std::io::Result<T> + Send + 'static) -> std::io::Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| std::io::Error::other(e.to_string()))?
}

/// The compactor: one at a time, in the background. At start, then each time it is woken (a
/// snapshot taken or deleted): every snapshot still a raw clone is compacted, then the chunks no
/// snapshot uses any more are deleted.
pub async fn run_compactor(ctx: Arc<AppCtx>) {
    loop {
        for meta in ctx.snapshots.list() {
            if ctx.snapshots.has_raw_disk(&meta.id)
                && let Err(e) = compact(&ctx, &meta.id).await
            {
                tracing::warn!(snapshot = %meta.id, error = %e, "snapshot not compacted (kept as a clone)");
            }
        }
        collect_garbage(&ctx).await;
        ctx.compactor.notified().await;
    }
}

/// Compacts snapshot `id` (no-op once done). Holds the store's lock: a cleanup never deletes the
/// chunks of a disk being compacted before its manifest is saved.
pub async fn compact(ctx: &AppCtx, id: &SnapshotId) -> std::io::Result<()> {
    let _lock = ctx.chunk_lock.lock().await;
    if !ctx.snapshots.has_raw_disk(id) {
        return Ok(());
    }
    let (chunks, disk, t) = (ctx.chunks.clone(), ctx.snapshots.disk(id), std::time::Instant::now());
    let manifest = blocking(move || chunks.pack(&disk)).await?;
    ctx.snapshots.compacted(id, &manifest)?;
    tracing::info!(snapshot = %id, chunks = manifest.chunks.len(), ms = t.elapsed().as_millis() as u64, "snapshot compacted");
    Ok(())
}

/// Deletes the chunks no snapshot uses (after a snapshot was deleted).
pub async fn collect_garbage(ctx: &AppCtx) {
    let _lock = ctx.chunk_lock.lock().await;
    let (chunks, snapshots) = (ctx.chunks.clone(), ctx.snapshots.clone());
    let freed = blocking(move || {
        let kept: Vec<_> = snapshots.manifests().into_iter().map(|(_, m)| m).collect();
        let garbage = unreferenced(chunks.present()?.into_keys(), &kept);
        chunks.remove(&garbage)
    })
    .await;
    match freed {
        Ok(0) => {}
        Ok(bytes) => tracing::info!(freed_mb = bytes >> 20, "unused snapshot chunks deleted"),
        Err(e) => tracing::warn!(error = %e, "unused snapshot chunks not deleted"),
    }
}

/// Writes snapshot `id`'s disk to `dest`: an instant clone while it is not compacted yet, else
/// restored from its chunks (every core; each chunk checked against its hash).
pub async fn restore_disk(ctx: &AppCtx, id: &SnapshotId, dest: PathBuf) -> std::io::Result<()> {
    let (chunks, snapshots, sid) = (ctx.chunks.clone(), ctx.snapshots.clone(), id.clone());
    blocking(move || match snapshots.manifest(&sid) {
        Some(m) => chunks.unpack(&m, &dest),
        // Compacted just now (the clone went after its manifest was saved): from the chunks.
        None => snapshots.clone_disk_to(&sid, &dest).or_else(|e| match snapshots.manifest(&sid) {
            Some(m) if e.kind() == std::io::ErrorKind::NotFound => chunks.unpack(&m, &dest),
            _ => Err(e),
        }),
    })
    .await
}

/// What each compacted snapshot really costs (MB its deletion frees), and all of them together.
pub fn sizes(ctx: &AppCtx) -> (HashMap<SnapshotId, u64>, u64) {
    let sizes = ctx.chunks.present().unwrap_or_default();
    let manifests = ctx.snapshots.manifests();
    let refs: Vec<_> = manifests.iter().map(|(id, m)| (id, m)).collect();
    let own = own_bytes(&refs, &sizes).into_iter().map(|(id, b)| (id.clone(), b >> 20)).collect();
    (own, sizes.values().sum::<u64>() >> 20)
}
