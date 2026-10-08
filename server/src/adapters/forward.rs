//! Port forwarding: `127.0.0.1:<host port>` on the Mac → a TCP port inside the VM, through the
//! VM's vsock bridge (works even for services bound to the VM's localhost).

use std::path::PathBuf;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, UnixStream};

/// Stops forwarding when dropped.
pub struct PortForward {
    pub host_port: u16,
    task: tokio::task::JoinHandle<()>,
}

impl PortForward {
    /// Prefers the same port on the Mac; falls back to any free port.
    pub fn start(guest_port: u16, socket: PathBuf) -> std::io::Result<Self> {
        let std_listener = std::net::TcpListener::bind(("127.0.0.1", guest_port))
            .or_else(|_| std::net::TcpListener::bind(("127.0.0.1", 0)))?;
        std_listener.set_nonblocking(true)?;
        let host_port = std_listener.local_addr()?.port();
        let listener = TcpListener::from_std(std_listener)?;
        let task = tokio::spawn(async move {
            while let Ok((mut tcp, _)) = listener.accept().await {
                let socket = socket.clone();
                tokio::spawn(async move {
                    let Ok(mut vm) = UnixStream::connect(&socket).await else { return };
                    let header = format!("{{\"forward\":{guest_port}}}\n");
                    if vm.write_all(header.as_bytes()).await.is_ok() {
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
