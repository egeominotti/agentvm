//! Follows a growing file and emits its complete lines.

use std::path::PathBuf;
use std::time::Duration;

use futures::Stream;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::watch;

const POLL: Duration = Duration::from_millis(200);

/// Ends when `stop` becomes `true`: reads what is left and also emits the last line without `\n`.
pub fn tail_lines(path: PathBuf, mut stop: watch::Receiver<bool>) -> impl Stream<Item = String> {
    async_stream(move |tx| async move {
        let mut offset = 0u64;
        let mut pending = Vec::new();
        loop {
            let stopping = *stop.borrow();
            if let Ok(mut f) = tokio::fs::File::open(&path).await
                && f.seek(std::io::SeekFrom::Start(offset)).await.is_ok()
            {
                let mut buf = Vec::new();
                if let Ok(n) = f.read_to_end(&mut buf).await {
                    offset += n as u64;
                    pending.extend_from_slice(&buf);
                }
            }
            while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = pending.drain(..=pos).collect();
                if tx.send(String::from_utf8_lossy(&line[..pos]).into_owned()).await.is_err() {
                    return;
                }
            }
            if stopping {
                if !pending.is_empty() {
                    let _ = tx.send(String::from_utf8_lossy(&pending).into_owned()).await;
                }
                return;
            }
            let _ = tokio::time::timeout(POLL, stop.changed()).await;
        }
    })
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
