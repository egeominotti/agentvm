//! Launch defaults and frozen provider metadata use real queued submissions and persistence.
use crate::helpers::{ctx, git_repo};
use agentvm::app::agents::update_provider;
use agentvm::app::store::Store;
use agentvm::app::submission::{NewTask, submit};
use agentvm::domain::accounts::{AccountKind, AgentAuth};
use agentvm::domain::agents::{AgentKind, AgentSelection};
use agentvm::domain::providers::{ApiProtocol, Provider, ProviderConfig};
use agentvm::domain::settings::Model;
use agentvm::domain::snapshot::SnapshotId;

fn request(repo: &str, agent: Option<AgentSelection>) -> NewTask<'_> {
    NewTask {
        repo,
        prompt: "task".into(),
        base_ref: None,
        interactive: true,
        model: None,
        agent,
        claude_version: None,
        restore_from: Some(SnapshotId::parse("snap-20260101-000000-abcd").unwrap()),
        cpus: Some(1),
        memory_mb: Some(1024),
        label: None,
        tailscale: Some(false),
    }
}
fn selection(kind: AgentKind, provider: Provider) -> AgentSelection {
    AgentSelection { kind, provider: Some(provider), ..Default::default() }
}

#[test]
fn switching_account_kinds_resets_an_inherited_model_alias() {
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    let mut settings = ctx.settings.get();
    settings.agent = AgentSelection {
        kind: AgentKind::Goose,
        auth: Some(AgentAuth::Account),
        account_kind: Some(AccountKind::Chatgpt),
        ..Default::default()
    };
    settings.model = Model::parse("gpt-old").unwrap();
    ctx.update_settings(settings).unwrap();
    let mut changed = ctx.settings.get();
    changed.agent.account_kind = Some(AccountKind::Claude);
    ctx.update_settings(changed).unwrap();
    assert_eq!(ctx.settings.get().model.as_str(), "default");
}

#[tokio::test]
async fn an_unconnected_account_cannot_queue_a_vm_with_guest_credentials() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let ctx = ctx(home.path());
    let agent = AgentSelection {
        kind: AgentKind::Codex,
        auth: Some(AgentAuth::Account),
        account_kind: Some(AccountKind::Chatgpt),
        ..Default::default()
    };
    let result = submit(&ctx, request(repo.path().to_str().unwrap(), Some(agent)));
    assert!(matches!(result, Err(agentvm::app::submission::SubmitError::Credential(_))), "{result:?}");
}
fn config(endpoint: &str, protocol: ApiProtocol) -> ProviderConfig {
    ProviderConfig { endpoint: endpoint.into(), protocol, requires_key: false }
}

#[tokio::test]
async fn omitted_agent_and_model_use_the_saved_required_model() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let ctx = ctx(home.path());
    let mut settings = ctx.settings.get();
    settings.agent = selection(AgentKind::Opencode, Provider::Local);
    settings.model = Model::parse("openai/gpt-default").unwrap();
    ctx.update_settings(settings).unwrap();
    // No await after submission: the queued supervisor cannot launch a VM on this runtime.
    let id = submit(&ctx, request(repo.path().to_str().unwrap(), None)).unwrap();
    let record = ctx.store.get(&id).unwrap();
    assert_eq!(record.agent.kind, AgentKind::Opencode);
    assert_eq!(record.model.as_str(), "openai/gpt-default");
}

#[tokio::test]
async fn changed_cli_or_provider_uses_its_own_cli_default() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let ctx = ctx(home.path());
    update_provider(&ctx, Provider::Custom, config("https://example.org", ApiProtocol::Anthropic), None).unwrap();
    let mut settings = ctx.settings.get();
    settings.agent = selection(AgentKind::Codex, Provider::Local);
    settings.model = Model::parse("gpt-default").unwrap();
    ctx.update_settings(settings).unwrap();
    let id = submit(&ctx, request(repo.path().to_str().unwrap(), Some(selection(AgentKind::Claude, Provider::Custom))))
        .unwrap();
    assert_eq!(ctx.store.get(&id).unwrap().model.as_str(), "default");
}

#[tokio::test]
async fn explicit_builtin_provider_matches_its_implicit_default() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let ctx = ctx(home.path());
    update_provider(&ctx, Provider::Openai, config("https://example.org/v1", ApiProtocol::Openai), None).unwrap();
    let mut settings = ctx.settings.get();
    settings.agent = AgentSelection { kind: AgentKind::Codex, ..Default::default() };
    settings.model = Model::parse("gpt-default").unwrap();
    ctx.update_settings(settings).unwrap();
    let id = submit(&ctx, request(repo.path().to_str().unwrap(), Some(selection(AgentKind::Codex, Provider::Openai))))
        .unwrap();
    assert_eq!(ctx.store.get(&id).unwrap().model.as_str(), "gpt-default");
    let changed =
        submit(&ctx, request(repo.path().to_str().unwrap(), Some(selection(AgentKind::Codex, Provider::Local))))
            .unwrap();
    assert_eq!(ctx.store.get(&changed).unwrap().model.as_str(), "default");
}

#[tokio::test]
async fn queued_record_preserves_its_provider_config_after_settings_change() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let ctx = ctx(home.path());
    let original = config("http://127.0.0.1:1234/v1", ApiProtocol::Openai);
    update_provider(&ctx, Provider::Local, original.clone(), None).unwrap();
    let id = submit(&ctx, request(repo.path().to_str().unwrap(), Some(selection(AgentKind::Codex, Provider::Local))))
        .unwrap();
    let json = serde_json::to_value(ctx.store.get(&id).unwrap()).unwrap();
    assert_eq!(json["agent_provider_config"], serde_json::to_value(&original).unwrap());
    update_provider(&ctx, Provider::Local, config("http://127.0.0.1:1234/api/v1", ApiProtocol::Anthropic), None)
        .unwrap();
    let records = Store::load(&home.path().join("jobs"));
    let saved = serde_json::to_value(&records[0]).unwrap();
    assert_eq!(saved["agent_provider_config"], serde_json::to_value(original).unwrap());
    let current = config("http://127.0.0.1:1234/api/v1", ApiProtocol::Openai);
    update_provider(&ctx, Provider::Local, current.clone(), None).unwrap();
    let restored =
        submit(&ctx, request(repo.path().to_str().unwrap(), Some(selection(AgentKind::Codex, Provider::Local))))
            .unwrap();
    assert_eq!(ctx.store.get(&restored).unwrap().agent_provider_config, Some(current));
}

#[test]
fn credential_refresh_respects_frozen_key_requirements() {
    use agentvm::adapters::keychain::Keychain;
    use agentvm::app::agents::{read_credential, read_credential_with_config};
    use agentvm::secret::Secret;
    use std::process::Command;
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
        }
    }
    let home = tempfile::tempdir().unwrap();
    let kc = Cleanup(home.path().join("frozen-provider.keychain-db"));
    assert!(Command::new("security").args(["create-keychain", "-p", "x"]).arg(&kc.0).status().unwrap().success());
    let mut ctx = ctx(home.path());
    std::sync::Arc::get_mut(&mut ctx).unwrap().keychain = Keychain::new(Some(kc.0.clone()));
    ctx.keychain.write_provider_key(Provider::Local, &Secret::new("current-host-key".into())).unwrap();
    let agent = selection(AgentKind::Codex, Provider::Local);
    let mut frozen = Provider::Local.config();
    frozen.requires_key = true;
    assert_eq!(read_credential(&ctx, &agent).unwrap().expose(), "");
    assert_eq!(read_credential_with_config(&ctx, &agent, Some(&frozen)).unwrap().expose(), "current-host-key");
    frozen.requires_key = false;
    update_provider(&ctx, Provider::Local, ProviderConfig { requires_key: true, ..frozen.clone() }, None).unwrap();
    assert_eq!(read_credential_with_config(&ctx, &agent, Some(&frozen)).unwrap().expose(), "");
    assert_eq!(read_credential(&ctx, &agent).unwrap().expose(), "current-host-key");
}

#[test]
fn legacy_task_records_have_no_frozen_provider_config() {
    let repo = git_repo();
    let mut value = serde_json::to_value(crate::helpers::record(&repo)).unwrap();
    value.as_object_mut().unwrap().remove("agent_provider_config");
    let record: agentvm::app::record::TaskRecord = serde_json::from_value(value).unwrap();
    assert!(record.agent_provider_config.is_none());
}
