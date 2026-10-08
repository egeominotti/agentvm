//! Portable snapshot archives: `tar` compressed with zstd (gzip when zstd is missing).

use std::path::Path;
use std::process::Command;

fn tar(args: &[&std::ffi::OsStr]) -> std::io::Result<()> {
    let out = Command::new("tar").args(args).output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(String::from_utf8_lossy(&out.stderr).trim().to_owned()))
    }
}

fn has_zstd() -> bool {
    Command::new("sh").args(["-c", "command -v zstd"]).output().is_ok_and(|o| o.status.success())
}

/// Archives the contents of `dir` (sparse VM disks stay small).
pub fn pack(dir: &Path, file: &Path) -> std::io::Result<()> {
    let compression = if has_zstd() { "--zstd" } else { "-z" };
    tar(&["-cf".as_ref(), file.as_os_str(), compression.as_ref(), "-C".as_ref(), dir.as_os_str(), ".".as_ref()])
}

/// Extracts into `dir` (created if needed); compression is detected automatically.
pub fn unpack(file: &Path, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    tar(&["-xf".as_ref(), file.as_os_str(), "-C".as_ref(), dir.as_os_str()])
}
