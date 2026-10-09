//! The server's background loops, kept alive: a loop that panics is logged and started again,
//! instead of stopping automatic snapshots (or record retries) until the next restart.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use super::context::AppCtx;

/// Starts every background loop.
pub fn start(ctx: &Arc<AppCtx>) {
    let c = ctx.clone();
    keep_alive("automatic snapshots", move || super::snapshots::run_schedule(c.clone()));
    let c = ctx.clone();
    keep_alive("record retries", move || retry_records(c.clone()));
}

/// Runs `make()` for ever: when the loop it returns ends or panics, it is logged and run again.
fn keep_alive<F, Fut>(name: &'static str, make: F)
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            match tokio::spawn(make()).await {
                Ok(()) => tracing::error!(name, "background loop ended: starting it again"),
                Err(e) => tracing::error!(name, error = %e, "background loop panicked: starting it again"),
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

/// Every 5 seconds: the task records a full disk kept from being written.
async fn retry_records(ctx: Arc<AppCtx>) {
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let store = ctx.store.clone_handle();
        let _ = tokio::task::spawn_blocking(move || store.retry_unsaved()).await;
    }
}
