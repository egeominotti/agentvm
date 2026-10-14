//! Agent catalog, selected provider configuration and host-only credentials.
use super::context::AppCtx;
use crate::domain::agents::{AgentDescriptor, AgentKind, AgentSelection};
use crate::domain::providers::{Provider, ProviderConfig};
use crate::secret::Secret;
use serde::Serialize;

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ProviderDescriptor {
    pub id: Provider,
    pub name: String,
    pub config: ProviderConfig,
    pub has_key: bool,
}
#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AgentCatalog {
    pub agents: Vec<AgentDescriptor>,
    pub providers: Vec<ProviderDescriptor>,
    pub claude_token: bool,
    pub accounts: Vec<crate::domain::accounts::AccountStatus>,
}

pub fn catalog(ctx: &AppCtx) -> AgentCatalog {
    let settings = ctx.settings.get();
    let providers: Vec<_> = Provider::ALL
        .into_iter()
        .map(|id| ProviderDescriptor {
            id,
            name: id.name().into(),
            config: settings.providers.get(&id).cloned().unwrap_or_else(|| id.config()),
            has_key: ctx.keychain.has_provider_key(id),
        })
        .collect();
    let agents = AgentKind::ALL
        .into_iter()
        .map(|kind| {
            let mut descriptor = kind.descriptor();
            descriptor.providers = providers
                .iter()
                .filter(|p| {
                    (p.id == Provider::Custom && p.config.endpoint.is_empty()) || kind.supports_config(&p.config)
                })
                .map(|p| p.id)
                .collect();
            descriptor
        })
        .collect();
    AgentCatalog {
        agents,
        providers,
        claude_token: ctx.keychain.token_status().is_ok(),
        accounts: crate::domain::accounts::AccountKind::ALL
            .into_iter()
            .map(|id| super::accounts::status(ctx, id))
            .collect(),
    }
}
pub fn provider_config(ctx: &AppCtx, selection: &AgentSelection) -> Option<ProviderConfig> {
    if let Some(account) = selection.resolved_account() {
        return Some(super::accounts::config(account));
    }
    selection.resolved_provider().map(|p| ctx.settings.get().providers.get(&p).cloned().unwrap_or_else(|| p.config()))
}
pub fn read_credential(ctx: &AppCtx, selection: &AgentSelection) -> Result<Secret, String> {
    read_credential_with_config(ctx, selection, None)
}
pub fn read_credential_with_config(
    ctx: &AppCtx,
    selection: &AgentSelection,
    frozen: Option<&ProviderConfig>,
) -> Result<Secret, String> {
    if let Some(account) = selection.resolved_account() {
        if account == crate::domain::accounts::AccountKind::Claude
            && let Ok(token) = ctx.keychain.read_token()
        {
            return Ok(token);
        }
        let status = super::accounts::status(ctx, account);
        return if status.connected {
            Ok(Secret::new(String::new()))
        } else {
            Err(format!("{} missing: connect it in Settings › Accounts", account.name()))
        };
    }
    let Some(provider) = selection.resolved_provider() else {
        return if selection.kind == AgentKind::Claude {
            ctx.keychain.read_token().map_err(|e| e.to_string())
        } else {
            Ok(Secret::new(String::new()))
        };
    };
    let current = frozen.is_none().then(|| provider_config(ctx, selection)).flatten();
    if frozen.or(current.as_ref()).is_some_and(|c| !c.requires_key) {
        return Ok(Secret::new(String::new()));
    }
    ctx.keychain
        .read_provider_key(provider)
        .ok_or_else(|| format!("{} API key missing: save it in Settings", provider.name()))
}
pub fn update_provider(
    ctx: &AppCtx,
    provider: Provider,
    config: ProviderConfig,
    token: Option<String>,
) -> Result<(), String> {
    config.validate()?;
    let changed = token.is_some();
    ctx.settings
        .set_provider_with(
            provider,
            config,
            || {
                let previous = if let Some(token) = token {
                    let previous = ctx.keychain.read_stored_provider_key(provider).map_err(|e| e.to_string())?;
                    ctx.keychain.write_provider_key(provider, &Secret::new(token)).map_err(|e| e.to_string())?;
                    previous
                } else {
                    None
                };
                Ok(previous)
            },
            |previous| {
                if !changed {
                    return Ok(());
                }
                match previous {
                    Some(previous) => ctx.keychain.write_provider_key(provider, &previous),
                    None => ctx.keychain.delete_provider_key(provider),
                }
                .map_err(|e| e.to_string())
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn delete_provider_credential(ctx: &AppCtx, provider: Provider) -> Result<(), String> {
    ctx.keychain.delete_provider_key(provider).map_err(|e| e.to_string())
}
