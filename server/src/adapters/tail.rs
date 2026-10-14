//! Follows growing guest files by byte position, detecting truncation and replacement.
mod lines;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::watch;

const POLL: Duration = Duration::from_millis(200);
const CHUNK: u64 = 1 << 20;
pub const MAX_LINE: usize = 8 << 20;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cursor {
    pub offset: u64,
    device: u64,
    inode: u64,
    anchor: String,
}

pub struct TailRecord {
    pub line: String,
    pub cursor: Cursor,
}

pub fn tail_lines(path: PathBuf, stop: watch::Receiver<bool>) -> impl Stream<Item = String> {
    tail_records(path, Cursor::default(), stop).map(|record| record.line)
}

/// Resume only when the file identity and bytes before the checkpoint still match.
pub fn tail_records(
    path: PathBuf,
    mut cursor: Cursor,
    mut stop: watch::Receiver<bool>,
) -> impl Stream<Item = TailRecord> {
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(async move {
        let mut lines = lines::Lines::default();
        let mut initialized = false;
        loop {
            let stopping = *stop.borrow();
            if let Some(mut file) = crate::guestfs::open_regular(&path) {
                use std::os::unix::fs::MetadataExt;
                let Ok(meta) = file.metadata() else { break };
                let trailer = read_anchor(&mut file, cursor.offset);
                let valid = cursor.offset == 0
                    || (meta.len() >= cursor.offset
                        && meta.dev() == cursor.device
                        && meta.ino() == cursor.inode
                        && trailer
                            .as_ref()
                            .is_some_and(|bytes| blake3::hash(bytes).to_hex().as_str() == cursor.anchor));
                if !valid {
                    cursor.offset = 0;
                    lines = lines::Lines::default();
                    initialized = false;
                }
                if !initialized {
                    lines = lines::Lines::with_trailer(if valid { trailer.unwrap_or_default() } else { Vec::new() });
                    initialized = true;
                }
                cursor.device = meta.dev();
                cursor.inode = meta.ino();
                let mut file = tokio::fs::File::from_std(file);
                if file.seek(std::io::SeekFrom::Start(cursor.offset)).await.is_ok() {
                    loop {
                        let mut buf = Vec::new();
                        let Ok(n) = (&mut file).take(CHUNK).read_to_end(&mut buf).await else { break };
                        for (line, offset, anchor) in lines.push(&buf, cursor.offset) {
                            let checkpoint = Cursor { offset, anchor, ..cursor.clone() };
                            if tx.send(TailRecord { line, cursor: checkpoint }).await.is_err() {
                                return;
                            }
                        }
                        cursor.offset += n as u64;
                        cursor.anchor = lines.anchor();
                        if (n as u64) < CHUNK {
                            break;
                        }
                    }
                }
            }
            if stopping {
                if let Some(line) = lines.rest() {
                    let _ = tx.send(TailRecord { line, cursor }).await;
                }
                return;
            }
            let _ = tokio::time::timeout(POLL, stop.changed()).await;
        }
    });
    tokio_stream::wrappers::ReceiverStream::new(rx)
}

fn read_anchor(file: &mut std::fs::File, offset: u64) -> Option<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let start = offset.saturating_sub(64);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = vec![0; (offset - start) as usize];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}
