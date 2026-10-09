//! agentvm-server: wiring only.

use std::sync::Arc;

use agentvm::adapters::keychain::Keychain;
use agentvm::adapters::lock::InstanceLock;
use agentvm::adapters::orphans::cleanup_orphans;
use agentvm::app::supervisor::AppCtx;
use agentvm::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = Config::from_env();
    let level = std::env::var("AGENTVM_LOG").unwrap_or_else(|_| "info".into());
    let _log = agentvm::logging::init(&config.home.join("logs"), &level);
    if let Err(e) = agentvm::adapters::host::raise_open_files_limit() {
        eprintln!("could not raise the open files limit: {e}");
    }
    // Lock and port first: only a single instance may touch orphaned VMs.
    let _lock = InstanceLock::acquire(&config.home)
        .map_err(|_| anyhow::anyhow!("agentvm is already running on {} (AGENTVM_HOME)", config.home.display()))?;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    // AGENTVM_PORT=0 lets the system pick a free port (tests); everything else uses the real one.
    let addr = listener.local_addr()?;
    config.port = addr.port();
    std::fs::create_dir_all(config.jobs())?;
    let ctx = Arc::new(AppCtx::new(config, Keychain::new(None)));
    // VMs survive restarts: re-attach to them first, then clean up whatever no task owns.
    let live = agentvm::app::supervisor::recover(&ctx);
    cleanup_orphans(&ctx.config.jobs(), &live);
    agentvm::app::backups::remove_leftovers(&ctx);
    agentvm::app::background::start(&ctx);
    eprintln!("agentvm listening on http://{addr}");
    axum::serve(listener, agentvm::http::router(ctx)).await?;
    Ok(())
}
