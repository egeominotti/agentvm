//! Tailscale: the auth key VMs join the user's tailnet with (saved in the Keychain, never shown),
//! and joining or leaving from a running VM's page.

use std::time::Duration;

use super::context::AppCtx;
use super::guest_channel::{Answer, AskError};
use crate::adapters::jobdir::JobWorkspace;
use crate::adapters::keychain::KeychainError;
use crate::domain::ids::TaskId;
use crate::domain::task::TaskState;
use crate::secret::Secret;

/// The guest starts joining at once (the join itself runs on, and reports in tailscale.json).
const START_TIMEOUT: Duration = Duration::from_secs(10);
/// Leaving waits for the logout, so the node goes from the tailnet before this returns.
const LEAVE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, thiserror::Error)]
pub enum TailscaleError {
    #[error("no such machine")]
    NotFound,
    #[error("the machine is not running: Tailscale joins and leaves from a running terminal")]
    NotRunning,
    #[error("save a Tailscale auth key first (Settings › Tailscale, or here)")]
    NoKey,
    #[error("that is not a Tailscale auth key (expected tskey-auth-… or tskey-client-…)")]
    InvalidKey,
    #[error("the Keychain refused: {0}")]
    Keychain(String),
    #[error("the VM did not answer in time")]
    Timeout,
    #[error("could not reach the VM: {0}")]
    Io(#[from] std::io::Error),
}

/// Whether a key is saved (the key itself is never sent anywhere but to a joining VM).
pub fn key_saved(ctx: &AppCtx) -> bool {
    ctx.keychain.read_tailscale_key().is_some()
}

pub fn save_key(ctx: &AppCtx, key: Secret) -> Result<(), TailscaleError> {
    ctx.keychain.write_tailscale_key(&key).map_err(keychain_error)
}

pub fn remove_key(ctx: &AppCtx) -> Result<(), TailscaleError> {
    ctx.keychain.delete_tailscale_key().map_err(keychain_error)
}

fn keychain_error(e: KeychainError) -> TailscaleError {
    match e {
        KeychainError::InvalidTailscaleKey => TailscaleError::InvalidKey,
        other => TailscaleError::Keychain(other.to_string()),
    }
}

/// Joins running VM `id` to the tailnet now, with the settings' SSH and tags.
pub async fn join(ctx: &AppCtx, id: &TaskId) -> Result<(), TailscaleError> {
    let record = running_terminal(ctx, id)?;
    let keychain = ctx.keychain.clone();
    let key = tokio::task::spawn_blocking(move || keychain.read_tailscale_key())
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .ok_or(TailscaleError::NoKey)?;
    let jobs = ctx.config.jobs();
    let share = JobWorkspace::share_of(&jobs, id);
    let spec = crate::domain::tailscale::spec_for(&super::proxy::vm_name(&record), &ctx.settings.get().tailscale);
    JobWorkspace::offer_tailnet(&share, &spec, Some(&key))?;
    ctx.store.set_tailscale(id, true);
    ask(ctx, id, "tailscale-up", "tailscale-up.started", START_TIMEOUT).await
}

/// Takes running VM `id` off the tailnet (its node goes at once).
pub async fn leave(ctx: &AppCtx, id: &TaskId) -> Result<(), TailscaleError> {
    running_terminal(ctx, id)?;
    ask(ctx, id, "tailscale-down", "tailscale-down.done", LEAVE_TIMEOUT).await?;
    JobWorkspace::forget_tailnet(&JobWorkspace::share_of(&ctx.config.jobs(), id));
    ctx.store.set_tailscale(id, false);
    Ok(())
}

fn running_terminal(ctx: &AppCtx, id: &TaskId) -> Result<super::record::TaskRecord, TailscaleError> {
    let record = ctx.store.get(id).ok_or(TailscaleError::NotFound)?;
    if !record.interactive || record.state != TaskState::Running {
        return Err(TailscaleError::NotRunning);
    }
    Ok(record)
}

async fn ask(
    ctx: &AppCtx,
    id: &TaskId,
    request: &str,
    reply: &'static str,
    timeout: Duration,
) -> Result<(), TailscaleError> {
    let not_running = || ctx.store.get(id).is_none_or(|r| r.state != TaskState::Running);
    match ctx.guest.ask(&ctx.config.jobs(), id, request, &[reply], timeout, not_running).await {
        Ok(Answer::Reply { .. }) => Ok(()),
        Ok(Answer::Gone) => Err(TailscaleError::NotRunning),
        Err(AskError::Timeout) => Err(TailscaleError::Timeout),
        Err(AskError::Io(e)) => Err(TailscaleError::Io(e)),
    }
}
