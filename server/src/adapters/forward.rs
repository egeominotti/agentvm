//! Reaching TCP ports inside a VM through its vsock bridge (this works even for services bound to
//! the VM's own localhost): a stream for the HTTP proxy, an HTTP probe, and direct TCP forwards.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpSocket, UnixStream};

/// A connection to `guest_port` inside the VM whose bridge listens on `socket`.
pub async fn connect(socket: &Path, guest_port: u16) -> io::Result<UnixStream> {
    let mut vm = UnixStream::connect(socket).await?;
    vm.write_all(format!("{{\"forward\":{guest_port}}}\n").as_bytes()).await?;
    Ok(vm)
}

/// Whether the service on `guest_port` answers HTTP (dev servers do; databases do not).
pub async fn speaks_http(socket: &Path, guest_port: u16) -> bool {
    let probe = async {
        let mut vm = connect(socket, guest_port).await?;
        vm.write_all(b"HEAD / HTTP/1.0\r\nHost: localhost\r\n\r\n").await?;
        let mut head = [0u8; 5];
        vm.read_exact(&mut head).await?;
        io::Result::Ok(&head == b"HTTP/")
    };
    tokio::time::timeout(Duration::from_secs(3), probe).await.is_ok_and(|r| r.unwrap_or(false))
}

/// Stops forwarding when dropped.
pub struct PortForward {
    pub host_port: u16,
    task: tokio::task::JoinHandle<()>,
}

impl PortForward {
    /// Always on a free port the system picks, shown in the dashboard: taking the VM's port number
    /// on the Mac (5432, 6379…) would let the VM receive what local clients send to a service of
    /// this Mac started later, passwords included.
    pub fn start(guest_port: u16, socket: PathBuf) -> io::Result<Self> {
        let s = TcpSocket::new_v4()?;
        s.bind(([127, 0, 0, 1], 0).into())?;
        let listener = s.listen(256)?;
        let host_port = listener.local_addr()?.port();
        let task = tokio::spawn(async move {
            loop {
                // An error here (out of file descriptors, a connection reset while queued) is about
                // one connection: the forward keeps listening instead of dying for good.
                let Ok((mut tcp, _)) = listener.accept().await else {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                };
                let socket = socket.clone();
                tokio::spawn(async move {
                    if let Ok(mut vm) = connect(&socket, guest_port).await {
                        let _ = tokio::io::copy_bidirectional(&mut tcp, &mut vm).await;
                    }
                });
            }
        });
        Ok(PortForward { host_port, task })
    }
}

impl Drop for PortForward {
    fn drop(&mut self) {
        self.task.abort();
    }
}
