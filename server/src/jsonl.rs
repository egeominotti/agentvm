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
    let mut file = std::fs::OpenOptions::new().append(true).create(true).custom_flags(O_NOFOLLOW).open(path)?;
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    // One write per line: a crash leaves at most one partial line, which `read` skips.
    file.write_all(&line)
}

/// Every value of the file, oldest first (from its last 32 MB; a partial line is skipped).
pub fn read<T: DeserializeOwned>(path: &Path) -> Vec<T> {
    let Ok(mut file) = std::fs::OpenOptions::new().read(true).custom_flags(O_NOFOLLOW).open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map_or(0, |m| m.len());
    let _ = file.seek(SeekFrom::Start(len.saturating_sub(READ_MAX)));
    let mut text = String::new();
    let _ = file.take(READ_MAX).read_to_string(&mut text);
    text.lines().filter_map(|l| serde_json::from_str::<T>(l).ok()).collect()
}
