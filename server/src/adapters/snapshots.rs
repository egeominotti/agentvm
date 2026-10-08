//! Snapshots on disk: `<root>/<id>/{disk.raw, efivars, meta.json}`.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::domain::snapshot::{SnapshotId, SnapshotMeta};

unsafe extern "C" {
    fn clonefile(src: *const std::ffi::c_char, dst: *const std::ffi::c_char, flags: u32) -> std::ffi::c_int;
}

/// Instant copy-on-write copy (APFS `clonefile(2)`).
fn clone(src: &Path, dst: &Path) -> io::Result<()> {
    let s = CString::new(src.as_os_str().as_bytes())?;
    let d = CString::new(dst.as_os_str().as_bytes())?;
    // SAFETY: valid C strings for the duration of the call.
    if unsafe { clonefile(s.as_ptr(), d.as_ptr(), 0) } == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

pub struct SnapshotStore {
    root: PathBuf,
}

impl SnapshotStore {
    pub fn new(root: PathBuf) -> Self {
        SnapshotStore { root }
    }

    fn dir(&self, id: &SnapshotId) -> PathBuf {
        self.root.join(id.as_str())
    }
    pub fn disk(&self, id: &SnapshotId) -> PathBuf {
        self.dir(id).join("disk.raw")
    }
    pub fn efivars(&self, id: &SnapshotId) -> PathBuf {
        self.dir(id).join("efivars")
    }

    /// Clones the disk (instant on APFS) and copies the EFI variables next to the metadata.
    pub fn create(&self, meta: &SnapshotMeta, disk: &Path, efivars: &Path) -> io::Result<SnapshotMeta> {
        let dir = self.dir(&meta.id);
        fs::create_dir_all(&dir)?;
        let result = (|| {
            // The guest flushed its cache; make sure the host has written it to the file too.
            fs::File::open(disk)?.sync_all()?;
            clone(disk, &self.disk(&meta.id))?;
            fs::copy(efivars, self.efivars(&meta.id))?;
            let saved = SnapshotMeta { size_mb: allocated_mb(&dir), ..meta.clone() };
            fs::write(dir.join("meta.json"), serde_json::to_vec_pretty(&saved)?)?;
            Ok(saved)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&dir);
        }
        result
    }

    pub fn get(&self, id: &SnapshotId) -> Option<SnapshotMeta> {
        let mut meta: SnapshotMeta = serde_json::from_slice(&fs::read(self.dir(id).join("meta.json")).ok()?).ok()?;
        meta.size_mb = allocated_mb(&self.dir(id));
        Some(meta)
    }

    /// Newest first.
    pub fn list(&self) -> Vec<SnapshotMeta> {
        let Ok(entries) = fs::read_dir(&self.root) else { return Vec::new() };
        let mut all: Vec<SnapshotMeta> = entries
            .flatten()
            .filter_map(|e| SnapshotId::parse(&e.file_name().to_string_lossy()))
            .filter_map(|id| self.get(&id))
            .collect();
        all.sort_by(|a, b| b.created_at.total_cmp(&a.created_at));
        all
    }

    /// Folder of a snapshot, e.g. to archive it.
    pub fn folder(&self, id: &SnapshotId) -> PathBuf {
        self.dir(id)
    }

    /// Scratch folder inside the store (same volume, so a rename is instant).
    pub fn scratch(&self, name: &str) -> io::Result<PathBuf> {
        let dir = self.root.join(format!(".{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Turns an extracted folder (with `disk.raw`, `efivars`, `meta.json`) into the snapshot `meta.id`.
    pub fn adopt(&self, dir: &Path, meta: SnapshotMeta) -> io::Result<SnapshotMeta> {
        if !dir.join("disk.raw").is_file() || !dir.join("efivars").is_file() {
            return Err(io::Error::other("the archive is not an agentvm snapshot"));
        }
        fs::write(dir.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
        fs::create_dir_all(&self.root)?;
        fs::rename(dir, self.dir(&meta.id))?;
        Ok(self.get(&meta.id).unwrap_or(meta))
    }

    pub fn delete(&self, id: &SnapshotId) -> io::Result<()> {
        fs::remove_dir_all(self.dir(id))
    }
}

fn allocated_mb(dir: &Path) -> u64 {
    fs::read_dir(dir).map_or(0, |entries| entries.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.blocks() * 512).sum::<u64>() >> 20)
}
