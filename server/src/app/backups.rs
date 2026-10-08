//! Moving snapshots off the Mac: download/upload archives and S3 backups.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

use super::supervisor::{AppCtx, random_bytes};
use crate::adapters::archive;
use crate::adapters::s3::S3Client;
use crate::domain::s3::S3Config;
use crate::domain::snapshot::{SnapshotId, SnapshotMeta};
use crate::secret::Secret;

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("snapshot not found")]
    NoSnapshot,
    #[error("S3 is not configured: set it up in Settings")]
    NotConfigured,
    #[error("{0}")]
    Invalid(String),
    #[error("S3: {0}")]
    S3(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteBackup {
    pub snapshot: SnapshotMeta,
    pub archive_mb: u64,
    pub uploaded_at: String,
    /// Already present among the local snapshots.
    pub local: bool,
}

const ARCHIVE: &str = "tar.zst";

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, BackupError> + Send + 'static) -> Result<T, BackupError> {
    tokio::task::spawn_blocking(f).await.map_err(|e| BackupError::Io(std::io::Error::other(e.to_string())))?
}

fn temp_dir(ctx: &AppCtx) -> std::io::Result<PathBuf> {
    let dir = ctx.config.home.join("tmp");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Writes `<snapshot>.tar.zst` in a temporary folder and returns its path.
pub async fn export(ctx: &AppCtx, sid: &SnapshotId) -> Result<PathBuf, BackupError> {
    if ctx.snapshots.get(sid).is_none() {
        return Err(BackupError::NoSnapshot);
    }
    let src = ctx.snapshots.folder(sid);
    let file = temp_dir(ctx)?.join(format!("{sid}.{ARCHIVE}"));
    let out = file.clone();
    blocking(move || Ok(archive::pack(&src, &out)?)).await?;
    Ok(file)
}

/// Adds an archive as a local snapshot. Keeps its id unless that id already exists here.
pub async fn import(ctx: &AppCtx, file: &Path) -> Result<SnapshotMeta, BackupError> {
    let scratch = ctx.snapshots.scratch(&format!("import-{}", std::process::id()))?;
    let (from, to) = (file.to_path_buf(), scratch.clone());
    blocking(move || Ok(archive::unpack(&from, &to)?)).await?;
    let meta: SnapshotMeta = serde_json::from_slice(&std::fs::read(scratch.join("meta.json"))?)
        .map_err(|_| BackupError::Invalid("the archive is not an agentvm snapshot".into()))?;
    let id = match SnapshotId::parse(meta.id.as_str()) {
        Some(id) if ctx.snapshots.get(&id).is_none() => id,
        _ => SnapshotId::generate(SystemTime::now(), random_bytes()),
    };
    let adopted = ctx.snapshots.adopt(&scratch, SnapshotMeta { id, ..meta });
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(adopted?)
}

pub fn s3_client(ctx: &AppCtx) -> Result<S3Client, BackupError> {
    let cfg = ctx.settings.get().s3.ok_or(BackupError::NotConfigured)?;
    let secret = ctx.keychain.read_s3_secret().map_err(|_| BackupError::NotConfigured)?;
    Ok(S3Client::new(cfg, secret))
}

/// Saves S3 settings (and the secret key, when given) after checking they work.
pub async fn configure_s3(ctx: &AppCtx, cfg: S3Config, secret: Option<String>) -> Result<(), BackupError> {
    cfg.validate().map_err(|e| BackupError::Invalid(e.to_string()))?;
    let secret = match secret.filter(|s| !s.trim().is_empty()) {
        Some(s) => Secret::new(s.trim().to_owned()),
        None => ctx.keychain.read_s3_secret().map_err(|_| BackupError::Invalid("enter the secret key".into()))?,
    };
    let probe = S3Client::new(cfg.clone(), Secret::new(secret.expose().to_owned()));
    blocking(move || check(&probe)).await?;
    ctx.keychain.write_s3_secret(&secret).map_err(|e| BackupError::Invalid(e.to_string()))?;
    let mut settings = ctx.settings.get();
    settings.s3 = Some(cfg);
    ctx.update_settings(settings).map_err(|e| BackupError::Invalid(e.to_string()))?;
    Ok(())
}

/// Creates the bucket if needed, then writes, reads and deletes a small object.
fn check(s3: &S3Client) -> Result<(), BackupError> {
    let e = |e: crate::adapters::s3::S3Error| BackupError::S3(e.to_string());
    s3.ensure_bucket().map_err(e)?;
    let key = s3.config().key(".agentvm-check");
    s3.put_bytes(&key, b"ok").map_err(e)?;
    if s3.get_bytes(&key).map_err(e)? != b"ok" {
        return Err(BackupError::S3("the test object came back different".into()));
    }
    s3.delete(&key).map_err(e)
}

pub async fn test_s3(ctx: &AppCtx) -> Result<(), BackupError> {
    let s3 = s3_client(ctx)?;
    blocking(move || check(&s3)).await
}

/// Uploads `<id>.tar.zst` and `<id>.json` (the metadata, for listing without downloading).
pub async fn backup(ctx: &AppCtx, sid: &SnapshotId) -> Result<RemoteBackup, BackupError> {
    let s3 = s3_client(ctx)?;
    let meta = ctx.snapshots.get(sid).ok_or(BackupError::NoSnapshot)?;
    let file = export(ctx, sid).await?;
    let id = sid.clone();
    let json = serde_json::to_vec_pretty(&meta).map_err(|e| BackupError::Io(e.into()))?;
    let result = blocking(move || {
        let e = |e: crate::adapters::s3::S3Error| BackupError::S3(e.to_string());
        s3.ensure_bucket().map_err(e)?;
        s3.put_file(&s3.config().key(&format!("{id}.{ARCHIVE}")), &file).map_err(e)?;
        s3.put_bytes(&s3.config().key(&format!("{id}.json")), &json).map_err(e)?;
        let size = std::fs::metadata(&file).map(|m| m.len() >> 20).unwrap_or(0);
        let _ = std::fs::remove_file(&file);
        Ok(size)
    })
    .await;
    let archive_mb = result?;
    Ok(RemoteBackup { snapshot: meta, archive_mb, uploaded_at: String::new(), local: true })
}

pub async fn list(ctx: &AppCtx) -> Result<Vec<RemoteBackup>, BackupError> {
    let s3 = s3_client(ctx)?;
    let local: Vec<String> = ctx.snapshots.list().into_iter().map(|m| m.id.to_string()).collect();
    let mut found = blocking(move || {
        let e = |e: crate::adapters::s3::S3Error| BackupError::S3(e.to_string());
        let objects = s3.list(&s3.config().key("")).map_err(e)?;
        let mut out = Vec::new();
        for obj in objects.iter().filter(|o| o.key.ends_with(".json") && o.key.contains("snap-")) {
            let Ok(meta) = serde_json::from_slice::<SnapshotMeta>(&s3.get_bytes(&obj.key).map_err(e)?) else { continue };
            let archive_key = obj.key.trim_end_matches(".json").to_owned() + "." + ARCHIVE;
            let Some(archive) = objects.iter().find(|o| o.key == archive_key) else { continue };
            out.push(RemoteBackup { archive_mb: archive.size >> 20, uploaded_at: archive.last_modified.clone(), local: false, snapshot: meta });
        }
        Ok(out)
    })
    .await?;
    for b in &mut found {
        b.local = local.contains(&b.snapshot.id.to_string());
    }
    found.sort_by(|a, b| b.snapshot.created_at.total_cmp(&a.snapshot.created_at));
    Ok(found)
}

/// Downloads a backup and adds it to the local snapshots.
pub async fn restore(ctx: &AppCtx, sid: &SnapshotId) -> Result<SnapshotMeta, BackupError> {
    let s3 = s3_client(ctx)?;
    let file = temp_dir(ctx)?.join(format!("{sid}.download.{ARCHIVE}"));
    let (key, out) = (s3.config().key(&format!("{sid}.{ARCHIVE}")), file.clone());
    blocking(move || s3.get_file(&key, &out).map_err(|e| BackupError::S3(e.to_string()))).await?;
    let result = import(ctx, &file).await;
    let _ = std::fs::remove_file(&file);
    result
}

pub async fn delete(ctx: &AppCtx, sid: &SnapshotId) -> Result<(), BackupError> {
    let s3 = s3_client(ctx)?;
    let id = sid.clone();
    blocking(move || {
        let e = |e: crate::adapters::s3::S3Error| BackupError::S3(e.to_string());
        s3.delete(&s3.config().key(&format!("{id}.{ARCHIVE}"))).map_err(e)?;
        s3.delete(&s3.config().key(&format!("{id}.json"))).map_err(e)
    })
    .await
}
