//! Small JSON files written atomically (see `atomic_file`).

use std::path::Path;

pub fn save<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    crate::atomic_file::write_json(path, value)
}
