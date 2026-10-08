//! Follows a growing file and emits its complete lines.

use std::path::PathBuf;
use std::time::Duration;

use futures::Stream;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::watch;

const POLL: Duration = Duration::from_millis(200);
/// Read at most this much at a time: memory stays bounded however fast the file grows.
const CHUNK: u64 = 1 << 20;
/// A longer line is dropped (with a short notice): the guest cannot make the host hold it.
pub const MAX_LINE: usize = 8 << 20;

/// Ends when `stop` becomes `true`: reads what is left and also emits the last line without `\n`.
pub fn tail_lines(path: PathBuf, mut stop: watch::Receiver<bool>) -> impl Stream<Item = String> {
    async_stream(move |tx| async move {
        let mut lines = Lines::default();
        let mut offset = 0u64;
        loop {
            let stopping = *stop.borrow();
            // The guest writes this file: never follow a symlink or block on a FIFO it planted.
            if let Some(f) = crate::guestfs::open_regular(&path)
                && let mut f = tokio::fs::File::from_std(f)
                && f.seek(std::io::SeekFrom::Start(offset)).await.is_ok()
            {
                loop {
                    let mut buf = Vec::new();
                    let Ok(n) = (&mut f).take(CHUNK).read_to_end(&mut buf).await else { break };
                    offset += n as u64;
                    for line in lines.push(&buf) {
                        if tx.send(line).await.is_err() {
                            return;
                        }
                    }
                    if (n as u64) < CHUNK {
                        break;
                    }
                }
            }
            if stopping {
                if let Some(last) = lines.rest() {
                    let _ = tx.send(last).await;
                }
                return;
            }
            let _ = tokio::time::timeout(POLL, stop.changed()).await;
        }
    })
}

/// Splits bytes into lines across reads, dropping any line longer than `MAX_LINE`.
#[derive(Default)]
struct Lines {
    pending: Vec<u8>,
    /// Bytes of an oversized line skipped so far (its end not seen yet).
    skipping: Option<usize>,
}

impl Lines {
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for part in bytes.split_inclusive(|&b| b == b'\n') {
            let (body, ends) = match part.strip_suffix(b"\n") {
                Some(body) => (body, true),
                None => (part, false),
            };
            if let Some(skipped) = self.skipping.as_mut() {
                *skipped += part.len();
            } else if self.pending.len() + body.len() > MAX_LINE {
                self.skipping = Some(self.pending.len() + part.len());
                self.pending.clear();
            } else {
                self.pending.extend_from_slice(body);
            }
            if ends {
                out.push(match self.skipping.take() {
                    Some(n) => format!("(agentvm skipped a line of {} MB)", n >> 20),
                    None => String::from_utf8_lossy(&std::mem::take(&mut self.pending)).into_owned(),
                });
            }
        }
        out
    }

    /// The last line, written without a final newline.
    fn rest(&mut self) -> Option<String> {
        (!self.pending.is_empty()).then(|| String::from_utf8_lossy(&std::mem::take(&mut self.pending)).into_owned())
    }
}

/// Stream fed by a tokio task through a channel.
fn async_stream<F, Fut>(f: F) -> impl Stream<Item = String>
where
    F: FnOnce(tokio::sync::mpsc::Sender<String>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(f(tx));
    tokio_stream::wrappers::ReceiverStream::new(rx)
}
