use super::connections::{load, persist_refreshed, read};
use super::{AccountAccess, AppCtx, index};
use crate::adapters::accounts;
use crate::domain::accounts::AccountKind;
use crate::domain::providers::{ApiProtocol, ProviderConfig};
use crate::secret::Secret;
use std::sync::Arc;

pub fn config(account: AccountKind) -> ProviderConfig {
    let (endpoint, protocol) = match account {
        AccountKind::Claude => ("https://api.anthropic.com", ApiProtocol::Anthropic),
        AccountKind::Chatgpt => ("https://chatgpt.com/backend-api/codex", ApiProtocol::Openai),
        AccountKind::Google => ("https://cloudcode-pa.googleapis.com", ApiProtocol::Google),
        AccountKind::Copilot => ("https://api.githubcopilot.com", ApiProtocol::Openai),
        AccountKind::Kilo => ("https://api.kilo.ai/api/openrouter", ApiProtocol::Openai),
    };
    ProviderConfig { endpoint: endpoint.into(), protocol, requires_key: true }
}
pub async fn access(ctx: &Arc<AppCtx>, account: AccountKind, force_refresh: bool) -> Result<AccountAccess, String> {
    let _guard = ctx.accounts.gates[index(account)].lock().await;
    let generation = ctx.accounts.control.lock().unwrap_or_else(|e| e.into_inner()).generations[index(account)];
    let mut config = config(account);
    if account == AccountKind::Claude {
        let ctx = ctx.clone();
        let credential = tokio::task::spawn_blocking(move || ctx.keychain.read_token())
            .await
            .map_err(|_| "Claude account worker failed")?
            .map_err(|_| "Claude account token missing")?;
        return Ok(AccountAccess { account, config, credential, catalog_credential: None, headers: vec![] });
    }
    let worker = ctx.clone();
    let (connection, mut session) = tokio::task::spawn_blocking(move || {
        let connection = load(&worker, account)?;
        let session = read(&worker, account, &connection)?;
        Ok::<_, String>((connection, session))
    })
    .await
    .map_err(|_| "account credential worker failed")??;
    let pending = ctx
        .accounts
        .pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&account)
        .filter(|(g, _)| *g == generation)
        .map(|(_, s)| s.clone());
    let recovered = pending.is_some();
    if let Some(next) = pending {
        let worker = ctx.clone();
        let saved = next.clone();
        tokio::task::spawn_blocking(move || persist_refreshed(&worker, account, generation, saved))
            .await
            .map_err(|_| "account save worker failed")??;
        session = next;
    }
    let expiring = session.expires_at != 0 && session.expires_at <= accounts::now() + 60;
    if connection.reference.is_some() {
        if expiring {
            return Err("referenced CLI account expired; sign in on the host again, then retry".into());
        }
    } else if (expiring || (force_refresh && !recovered))
        && matches!(account, AccountKind::Chatgpt | AccountKind::Google)
    {
        session = accounts::refresh(account, &session).await?;
        let next = session.clone();
        let worker = ctx.clone();
        tokio::task::spawn_blocking(move || persist_refreshed(&worker, account, generation, next))
            .await
            .map_err(|_| "account save worker failed")??;
    } else if expiring {
        return Err("account expired; sign in again".into());
    }
    let catalog_credential = account.catalog_credential(&session.access_token);
    if account == AccountKind::Copilot {
        let mut cached = ctx.accounts.copilot.lock().await;
        let reusable = !force_refresh
            && cached
                .as_ref()
                .is_some_and(|c| c.token == session.access_token && c.session.expires_at > accounts::now() + 60);
        if !reusable {
            let (minted, endpoint) = accounts::copilot_token(&session.access_token).await?;
            *cached = Some(super::CopilotCache { token: session.access_token.clone(), session: minted, endpoint });
        }
        let cache = cached.as_ref().expect("minted or cached");
        session = cache.session.clone();
        config.endpoint = cache.endpoint.clone();
    }
    let headers = if account == AccountKind::Chatgpt {
        vec![("ChatGPT-Account-ID".into(), session.account_id.ok_or("ChatGPT account identity missing")?)]
    } else if account == AccountKind::Kilo {
        session.account_id.map(|id| vec![("X-KILOCODE-ORGANIZATIONID".into(), id)]).unwrap_or_default()
    } else {
        vec![]
    };
    Ok(AccountAccess { account, config, credential: Secret::new(session.access_token), catalog_credential, headers })
}
