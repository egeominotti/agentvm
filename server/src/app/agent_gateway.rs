//! Inference relay configuration and lifetime for newly launched and recovered VMs.

use super::context::AppCtx;
use super::record::TaskRecord;
use crate::adapters::agent_proxy::{AccountRefresh, ProviderRelay};
use crate::adapters::jobdir::JobWorkspace;
use crate::domain::agents::AgentKind;
use crate::domain::providers::{Provider, ProviderConfig};
use crate::secret::Secret;
use std::sync::Arc;

pub fn config(ctx: &AppCtx, record: &TaskRecord) -> Option<ProviderConfig> {
    record
        .agent_provider_config
        .clone()
        .or_else(|| super::agents::provider_config(ctx, &record.agent))
        .or_else(|| (record.agent.kind == AgentKind::Claude).then(|| Provider::Anthropic.config()))
}

pub fn start(ctx: &Arc<AppCtx>, record: &TaskRecord, credential: &Secret) -> Result<Option<ProviderRelay>, String> {
    let Some(config) = config(ctx, record) else {
        return Ok(None);
    };
    let ws_path = JobWorkspace::agent_socket_of(&record.id);
    if let Some(account) = record.agent.resolved_account() {
        let ctx = ctx.clone();
        let refresh: AccountRefresh = Arc::new(move |force| {
            let ctx = ctx.clone();
            Box::pin(async move { super::accounts::access(&ctx, account, force).await })
        });
        return ProviderRelay::start_account(&ws_path, account, config, refresh).map(Some);
    }
    let oauth = record.agent.kind == AgentKind::Claude && credential.expose().starts_with("sk-ant-oat");
    ProviderRelay::start_for_agent(
        &ws_path,
        config,
        Secret::new(credential.expose().to_owned()),
        oauth,
        record.agent.kind,
        record.id.to_string(),
    )
    .map(Some)
}
