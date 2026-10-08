//! Connection to a terminal in the VM: helper Unix socket → vsock → guest PTY server.

use std::path::Path;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

/// Messages to the guest: `[type:1][length:4 BE][payload]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

pub fn encode(frame: &Frame) -> Vec<u8> {
    let (kind, payload) = match frame {
        Frame::Input(bytes) => (0u8, bytes.clone()),
        Frame::Resize { cols, rows } => (1u8, json!({ "cols": cols, "rows": rows }).to_string().into_bytes()),
    };
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(kind);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    out
}

pub struct PtyConnection {
    reader: PtyReader,
    writer: PtyWriter,
}

pub struct PtyReader(OwnedReadHalf);
pub struct PtyWriter(OwnedWriteHalf);

impl PtyConnection {
    /// Opens session `session` (`claude` or `shell`), creating it if it does not exist yet.
    pub async fn open(socket: &Path, session: &str, cols: u16, rows: u16) -> std::io::Result<Self> {
        let stream = UnixStream::connect(socket).await?;
        let (read, mut write) = stream.into_split();
        let header = json!({ "session": session, "cols": cols, "rows": rows }).to_string() + "\n";
        write.write_all(header.as_bytes()).await?;
        Ok(PtyConnection { reader: PtyReader(read), writer: PtyWriter(write) })
    }

    pub async fn send(&mut self, frame: &Frame) -> std::io::Result<()> {
        self.writer.send(frame).await
    }

    pub async fn recv(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        self.reader.recv().await
    }

    pub fn split(self) -> (PtyReader, PtyWriter) {
        (self.reader, self.writer)
    }
}

impl PtyReader {
    /// Raw terminal bytes; `None` when the session closes.
    pub async fn recv(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        let mut buf = vec![0u8; 16 * 1024];
        let n = self.0.read(&mut buf).await?;
        if n == 0 {
            return Ok(None);
        }
        buf.truncate(n);
        Ok(Some(buf))
    }
}

impl PtyWriter {
    pub async fn send(&mut self, frame: &Frame) -> std::io::Result<()> {
        self.0.write_all(&encode(frame)).await
    }
}
