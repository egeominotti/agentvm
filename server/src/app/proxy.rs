//! `http://<port>.<vm>.localhost:<server port>` → that port inside that VM.

use tokio::net::UnixStream;

use super::store::TaskRecord;
use super::supervisor::AppCtx;
use crate::adapters::forward;
use crate::adapters::jobdir::JobWorkspace;
use crate::domain::hostname;
use crate::domain::task::TaskState;

/// The machine's DNS name, from what the dashboard shows as its title.
pub fn vm_name(record: &TaskRecord) -> String {
    let title = match (&record.label, &record.prompt) {
        (Some(label), _) => label.trim_start_matches("Restored: ").to_owned(),
        (None, Some(p)) => p.as_str().lines().next().unwrap_or_default().to_owned(),
        (None, None) => record.repo.as_path().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
    };
    hostname::vm_name(&title, &record.id)
}

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("no running machine is called {0}")]
    NoSuchMachine(String),
    #[error("nothing answers on port {port} in {vm}: {source}")]
    Unreachable { vm: String, port: u16, source: std::io::Error },
}

/// A connection to `port` inside the running machine called `vm`.
pub async fn connect(ctx: &AppCtx, vm: &str, port: u16) -> Result<UnixStream, ProxyError> {
    let id = ctx
        .store
        .find_id(|r| r.interactive && r.state == TaskState::Running && vm_name(r) == vm)
        .ok_or_else(|| ProxyError::NoSuchMachine(vm.to_owned()))?;
    let socket = JobWorkspace::pty_socket_of(&ctx.config.jobs(), &id);
    forward::connect(&socket, port).await.map_err(|source| ProxyError::Unreachable { vm: vm.to_owned(), port, source })
}
