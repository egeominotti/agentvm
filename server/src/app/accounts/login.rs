use super::connections::save;
use super::{AppCtx, index};
use crate::adapters::accounts::{self, Connection, Session};
use crate::domain::accounts::{AccountKind, LoginStart, LoginState, LoginStatus};
use std::sync::Arc;

pub fn register_login(ctx: &AppCtx, account: AccountKind, start: &LoginStart) -> Result<(u64, u64), String> {
    let mut control = ctx.accounts.control.lock().unwrap_or_else(|e| e.into_inner());
    if control.logins.len() >= 64 {
        control.logins.retain(|_, (_, s)| matches!(s.status, LoginState::Pending));
    }
    if control.logins.len() >= 64 {
        return Err("too many pending account sign-ins".into());
    }
    control.login_generations[index(account)] += 1;
    let generation = (control.generations[index(account)], control.login_generations[index(account)]);
    control.logins.insert(
        start.id.clone(),
        (account, LoginStatus { id: start.id.clone(), status: LoginState::Pending, error: None }),
    );
    Ok(generation)
}

pub async fn begin_login(ctx: &Arc<AppCtx>, account: AccountKind) -> Result<LoginStart, String> {
    let (start, flow) = accounts::login::start(account).await?;
    let generation = register_login(ctx, account, &start)?;
    let ctx = ctx.clone();
    let id = start.id.clone();
    tokio::spawn(async move {
        let outcome = accounts::login::complete(flow).await;
        let _guard = ctx.accounts.gates[index(account)].lock().await;
        let worker = ctx.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let _ = finish_login(&worker, account, &id, generation, outcome);
        })
        .await;
    });
    Ok(start)
}

/// Commit a completed login only while its connection and attempt are still current.
pub fn finish_login(
    ctx: &AppCtx,
    account: AccountKind,
    id: &str,
    generation: (u64, u64),
    outcome: Result<Session, String>,
) -> Result<(), String> {
    let mut control = ctx.accounts.control.lock().unwrap_or_else(|e| e.into_inner());
    let current = (control.generations[index(account)], control.login_generations[index(account)]);
    let outcome = if current != generation {
        Err("account sign-in cancelled".into())
    } else {
        outcome.and_then(|session| save(ctx, account, &Connection { session: Some(session), reference: None }))
    };
    if outcome.is_ok() {
        control.generations[index(account)] += 1;
        ctx.accounts.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&account);
    }
    if let Some((_, status)) = control.logins.get_mut(id) {
        match &outcome {
            Ok(()) => status.status = LoginState::Connected,
            Err(error) => {
                status.status = LoginState::Failed;
                status.error = Some(error.clone());
            }
        }
    }
    outcome
}

pub fn login_status(ctx: &AppCtx, account: AccountKind, id: &str) -> Result<LoginStatus, String> {
    ctx.accounts
        .control
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .logins
        .get(id)
        .filter(|(a, _)| *a == account)
        .map(|(_, s)| s.clone())
        .ok_or_else(|| "account sign-in not found; start again".into())
}
