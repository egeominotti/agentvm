//! Reads the server's own log back: the lines about one VM, for its diagnostics.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Files are `agentvm.log.<YYYY-MM-DD>` (one a day).
const PREFIX: &str = "agentvm.log.";
/// Only the end of each file is read: a busy day's log can be large.
const TAIL_BYTES: u64 = 8 << 20;

/// The lines whose `fields.task` is `task`, from the newest two days, oldest first, at most
/// `max_lines` (the newest ones).
pub fn lines_for(logs_dir: &Path, task: &str, max_lines: usize) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(logs_dir) else { return Vec::new() };
    let mut days: Vec<_> =
        entries.flatten().filter(|e| e.file_name().to_string_lossy().starts_with(PREFIX)).map(|e| e.path()).collect();
    days.sort();
    let mut lines: Vec<String> = days
        .iter()
        .rev()
        .take(2)
        .rev()
        .flat_map(|day| tail(day))
        .filter(|line| {
            serde_json::from_str::<serde_json::Value>(line).is_ok_and(|v| v["fields"]["task"].as_str() == Some(task))
        })
        .collect();
    let skip = lines.len().saturating_sub(max_lines);
    lines.drain(..skip);
    lines
}

/// Whole lines of the last `TAIL_BYTES` of `path`.
fn tail(path: &Path) -> Vec<String> {
    let Ok(mut file) = std::fs::File::open(path) else { return Vec::new() };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut text = String::new();
    if file.take(TAIL_BYTES).read_to_string(&mut text).is_err() {
        return Vec::new();
    }
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0); // cut in the middle
    }
    lines
}
