//! Histories the host keeps next to a job (`telemetry.jsonl`, `usage.jsonl`): one JSON value
//! per line, appended for the VM's whole life. The job folder is the host's (the guest only
//! shares `share/`), and the files are still never opened through a symlink.

use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

const O_NOFOLLOW: i32 = 0x0100;
/// Months of 10 s lines: more is never read back.
const READ_MAX: u64 = 32 << 20;

pub fn append<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut file =
        std::fs::OpenOptions::new().read(true).append(true).create(true).custom_flags(O_NOFOLLOW).open(path)?;
    let mut line = Vec::new();
    // After a line cut short (full disk, power cut), start on a line of its own: glued to the
    // partial one, this line would be lost with it.
    if !ends_with_newline(&mut file)? {
        line.push(b'\n');
    }
    line.extend(serde_json::to_vec(value)?);
    line.push(b'\n');
    // One write per line: a crash leaves at most one partial line, which `read` skips.
    file.write_all(&line)
}

/// Whether the file is empty or ends with a newline.
fn ends_with_newline(file: &mut std::fs::File) -> std::io::Result<bool> {
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(true);
    }
    file.seek(SeekFrom::Start(len - 1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    Ok(last[0] == b'\n')
}

/// Every value of the file, oldest first (from its last 32 MB; a partial line is skipped).
pub fn read<T: DeserializeOwned>(path: &Path) -> Vec<T> {
    let Ok(mut file) = std::fs::OpenOptions::new().read(true).custom_flags(O_NOFOLLOW).open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map_or(0, |m| m.len());
    let _ = file.seek(SeekFrom::Start(len.saturating_sub(READ_MAX)));
    // Bytes, not a string: garbage left by an interrupted write costs its line, not the file.
    let mut bytes = Vec::new();
    let _ = file.take(READ_MAX).read_to_end(&mut bytes);
    bytes.split(|&b| b == b'\n').filter_map(|l| serde_json::from_slice::<T>(l).ok()).collect()
}

/// The file's last value, read from its last 64 KB (a value is a few hundred bytes).
pub fn last<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let mut file = std::fs::OpenOptions::new().read(true).custom_flags(O_NOFOLLOW).open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(64 << 10))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    bytes.split(|&b| b == b'\n').rev().find_map(|l| serde_json::from_slice::<T>(l).ok())
}
