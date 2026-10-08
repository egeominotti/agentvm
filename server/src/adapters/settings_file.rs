//! Settings persisted as JSON in `AGENTVM_HOME/settings.json`.

use std::path::{Path, PathBuf};

use crate::domain::settings::Settings;

/// What is on disk: read as plain JSON, so the caller can keep every field that still fits.
#[derive(Debug, PartialEq)]
pub enum Saved {
    Missing,
    /// Present but not JSON (a hand edit, a disk error): a copy is worth keeping.
    Corrupt,
    Json(serde_json::Value),
}

pub fn load(path: &Path) -> Saved {
    match std::fs::read(path) {
        Err(_) => Saved::Missing,
        Ok(bytes) => serde_json::from_slice(&bytes).map_or(Saved::Corrupt, Saved::Json),
    }
}

/// Keeps the file as it is next to it (`settings.json.bad`) before anything overwrites it.
pub fn keep_copy(path: &Path) -> std::io::Result<PathBuf> {
    let copy = path.with_extension("json.bad");
    std::fs::copy(path, &copy)?;
    Ok(copy)
}

/// Atomic write (temporary file + rename): a crash never leaves half a file.
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    std::fs::rename(tmp, path)
}
