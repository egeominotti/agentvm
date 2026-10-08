//! Claude's session files the guest copies to `share/claude/`, read a page of lines at a time:
//! never a whole file in memory, never a line past 1 MB, never through a symlink.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// A longer line (a tool that printed megabytes) is skipped: it still counts as a line.
const MAX_LINE: usize = 1 << 20;

#[derive(Debug, PartialEq)]
pub struct Page {
    pub lines: Vec<String>,
    /// The line to ask for next.
    pub next: usize,
}

/// Up to `max` lines from line `from`, counted across the session files oldest first.
pub fn read_page(dir: &Path, from: usize, max: usize) -> Page {
    let mut page = Page { lines: Vec::new(), next: from };
    let mut index = 0;
    for file in sessions(dir) {
        let Some(f) = crate::guestfs::open_regular(&file) else { continue };
        let mut reader = BufReader::new(f);
        while page.lines.len() < max {
            let Some(line) = next_line(&mut reader) else { break };
            if index >= from {
                if let Some(text) = line {
                    page.lines.push(text);
                }
                page.next = index + 1;
            }
            index += 1;
        }
        if page.lines.len() >= max {
            break;
        }
    }
    page
}

/// The `*.jsonl` regular files of `dir` (symlinks left out), oldest first.
fn sessions(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<_> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
        .filter_map(|e| {
            let meta = std::fs::symlink_metadata(e.path()).ok()?;
            meta.is_file().then(|| (meta.modified().ok(), e.path()))
        })
        .collect();
    files.sort();
    files.into_iter().map(|(_, p)| p).collect()
}

/// The next complete line: `None` at the end, `Some(None)` for a line too long to keep.
fn next_line(reader: &mut impl BufRead) -> Option<Option<String>> {
    let mut line = Vec::new();
    let mut too_long = false;
    loop {
        let buf = reader.fill_buf().ok()?;
        if buf.is_empty() {
            // A last line without its newline is still being copied: not a line yet.
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
        let used = ends.unwrap_or(buf.len());
        reader.consume(used);
        if ends.is_some() {
            return Some(finish(line, too_long));
        }
    }
}

fn finish(line: Vec<u8>, too_long: bool) -> Option<String> {
    (!too_long).then(|| String::from_utf8_lossy(&line).into_owned())
}
