//! Claude's session files the guest copies to `share/claude/`, read from where the last read
//! stopped: a cursor keeps a byte offset per file, so a read costs what is new, every line is
//! read once whichever file grew, and a page is bounded in bytes.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::guestfs::GuestDir;

/// A longer line (a tool that printed megabytes) is skipped: the cursor moves past it.
const MAX_LINE: usize = 1 << 20;
/// Session files looked at per read: far more than a VM's real sessions, few enough that a guest
/// creating thousands of files cannot make every read open them all.
const MAX_FILES: usize = 500;

/// Per session file, the offset of the first line not read yet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Cursor(pub BTreeMap<String, u64>);

/// The complete lines written since `cursor` (files in name order), up to about `max_bytes`,
/// and the cursor to continue from.
pub fn read_new(dir: &Path, cursor: &Cursor, max_bytes: u64) -> (Vec<String>, Cursor) {
    let mut next = cursor.clone();
    let mut lines = Vec::new();
    let Some(guest) = GuestDir::open(dir) else { return (lines, next) };
    let mut names: Vec<String> = guest.names().into_iter().filter(|n| n.ends_with(".jsonl")).collect();
    names.sort();
    names.truncate(MAX_FILES);
    let mut budget = max_bytes;
    for name in names {
        let Some(mut file) = guest.open_file(&name) else { continue };
        let len = file.metadata().map_or(0, |m| m.len());
        let mut offset = next.0.get(&name).copied().unwrap_or(0);
        // The copier rewrote a session that shrank: read it again from the start.
        if offset > len {
            offset = 0;
        }
        if file.seek(SeekFrom::Start(offset)).is_err() {
            continue;
        }
        let mut reader = BufReader::new(file);
        while budget > 0 {
            let Some((line, used)) = next_line(&mut reader) else { break };
            offset += used;
            budget = budget.saturating_sub(used);
            lines.extend(line);
        }
        // Only files something was read from: empty ones would grow the cursor for nothing.
        if offset > 0 {
            next.0.insert(name, offset);
        } else {
            next.0.remove(&name);
        }
        if budget == 0 {
            break;
        }
    }
    (lines, next)
}

/// The next complete line (`None` inside when too long to keep) and the bytes it took; `None`
/// at the end, including a last line still being copied (no newline yet).
fn next_line(reader: &mut impl BufRead) -> Option<(Option<String>, u64)> {
    let (mut line, mut used, mut too_long) = (Vec::new(), 0u64, false);
    loop {
        let buf = reader.fill_buf().ok()?;
        if buf.is_empty() {
            return None;
        }
        let (chunk, ends) = match buf.iter().position(|&b| b == b'\n') {
            Some(i) => (&buf[..i], Some(i + 1)),
            None => (buf, None),
        };
        if !too_long && line.len() + chunk.len() <= MAX_LINE {
            line.extend_from_slice(chunk);
        } else {
            too_long = true;
            line.clear();
        }
        let take = ends.unwrap_or(buf.len());
        reader.consume(take);
        used += take as u64;
        if ends.is_some() {
            return Some(((!too_long).then(|| String::from_utf8_lossy(&line).into_owned()), used));
        }
    }
}
