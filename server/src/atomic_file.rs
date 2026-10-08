//! Small files written whole or not at all: a crash, a kernel panic or two writers at once
//! never leave half a file behind. Used for every piece of state the server keeps on disk.

use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

unsafe extern "C" {
    fn fsync(fd: std::ffi::c_int) -> std::ffi::c_int;
}

/// Plain fsync: the data reaches the drive before the rename makes it visible. (Rust's
/// `sync_all` is a full-drive flush on macOS: far slower, and it stalls every VM's disk.)
fn flush(file: &std::fs::File) -> std::io::Result<()> {
    // SAFETY: a valid open descriptor for the duration of the call.
    if unsafe { fsync(file.as_raw_fd()) } == 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
}

/// Writes `bytes` to a temporary file of its own next to `path`, flushes it, then renames it
/// over `path`. Concurrent writers each rename a whole file; the last rename wins.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}-{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        flush(&file)?;
        std::fs::rename(&tmp, path)?;
        // The rename itself is in the folder: flush it too, so it survives a crash.
        std::fs::File::open(dir).and_then(|d| flush(&d))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// `value` as pretty JSON, written with [`write`].
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    write(path, &serde_json::to_vec_pretty(value)?)
}
