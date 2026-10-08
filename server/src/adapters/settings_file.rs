//! Settings persisted as JSON in `AGENTVM_HOME/settings.json`.

use std::path::Path;

use crate::domain::settings::Settings;

/// `None` when the file is missing or unreadable: the caller falls back to defaults.
pub fn load(path: &Path) -> Option<Settings> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Atomic write (temporary file + rename): a crash never leaves half a file.
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    std::fs::rename(tmp, path)
}
