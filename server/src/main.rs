//! agentvm-server: solo wiring.

use std::sync::Arc;

use agentvm::adapters::jobdir::cleanup_orphans;
use agentvm::adapters::keychain::Keychain;
use agentvm::adapters::lock::InstanceLock;
use agentvm::app::scheduler::Scheduler;
use agentvm::app::store::Store;
use agentvm::app::supervisor::AppCtx;
use agentvm::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env();
    // Prima il lock e la porta: solo un'istanza unica può toccare le VM rimaste orfane.
    let _lock = InstanceLock::acquire(&config.home).map_err(|_| {
        anyhow::anyhow!("agentvm è già in esecuzione su {} (AGENTVM_HOME)", config.home.display())
    })?;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    std::fs::create_dir_all(config.jobs())?;
    cleanup_orphans(&config.jobs());

    let ctx = Arc::new(AppCtx {
        scheduler: Scheduler::new(config.concurrency),
        store: Store::new(),
        keychain: Keychain::new(None),
        config,
    });
    eprintln!("agentvm in ascolto su http://{addr}");
    axum::serve(listener, agentvm::http::router(ctx)).await?;
    Ok(())
}
