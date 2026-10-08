//! agentvm-server: wiring only.

use std::sync::Arc;

use agentvm::adapters::jobdir::cleanup_orphans;
use agentvm::adapters::keychain::Keychain;
use agentvm::adapters::lock::InstanceLock;
use agentvm::app::supervisor::AppCtx;
use agentvm::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env();
    // Lock and port first: only a single instance may touch orphaned VMs.
    let _lock = InstanceLock::acquire(&config.home)
        .map_err(|_| anyhow::anyhow!("agentvm is already running on {} (AGENTVM_HOME)", config.home.display()))?;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    std::fs::create_dir_all(config.jobs())?;
    cleanup_orphans(&config.jobs());

    let ctx = Arc::new(AppCtx::new(config, Keychain::new(None)));
    eprintln!("agentvm listening on http://{addr}");
    axum::serve(listener, agentvm::http::router(ctx)).await?;
    Ok(())
}
