//! Settings at start-up: one wrong field never wipes the others (nor the S3 bucket).

use agentvm::app::settings::SettingsService;
use agentvm::domain::settings::{HostLimits, Model, Settings};
use serde_json::json;

fn defaults() -> Settings {
    Settings {
        max_vms: 4,
        cpus: 4,
        memory_mb: 4096,
        timeout_s: 1800,
        model: Model::default_choice(),
        default_repo: None,
        claude_version: Default::default(),
        s3: None,
        auto_snapshots: Default::default(),
        tailscale: Default::default(),
        agent: Default::default(),
        providers: Default::default(),
        custom_cli: None,
    }
}

#[test]
fn a_saved_value_this_mac_cannot_run_resets_only_that_value() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    let mut saved = serde_json::to_value(defaults()).unwrap();
    saved["cpus"] = json!(32);
    saved["max_vms"] = json!(9);
    saved["s3"] = json!({"endpoint": "https://s3.example.com", "region": "eu", "bucket": "agentvm-backups",
                         "access_key": "AK", "prefix": "agentvm/", "path_style": false});
    std::fs::write(&path, saved.to_string()).unwrap();
    let service = SettingsService::load(path.clone(), defaults(), HostLimits { cpus: 8, ram_mb: 32768 });
    let s = service.get();
    assert_eq!((s.cpus, s.max_vms), (4, 9));
    assert_eq!(s.s3.as_ref().map(|c| c.bucket.as_str()), Some("agentvm-backups"), "the bucket survives");
    assert!(tmp.path().join("settings.json.bad").is_file(), "the file as it was is kept");
}

#[test]
fn a_file_that_is_not_json_gives_the_defaults_and_is_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    std::fs::write(&path, "not json").unwrap();
    let service = SettingsService::load(path, defaults(), HostLimits { cpus: 8, ram_mb: 32768 });
    assert_eq!(service.get(), defaults());
    assert_eq!(std::fs::read_to_string(tmp.path().join("settings.json.bad")).unwrap(), "not json");
}

/// Two saves at once: the scheduler ends with the value that was saved last, never the other.
#[tokio::test(flavor = "multi_thread")]
async fn the_scheduler_follows_the_settings_saved_last() {
    let home = tempfile::tempdir().unwrap();
    let ctx = crate::helpers::ctx(home.path());
    for _ in 0..50 {
        let writers: Vec<_> = [3usize, 9]
            .into_iter()
            .map(|n| {
                let ctx = ctx.clone();
                tokio::task::spawn_blocking(move || {
                    let mut s = ctx.settings.get();
                    s.max_vms = n;
                    ctx.update_settings(s).unwrap();
                })
            })
            .collect();
        for w in writers {
            w.await.unwrap();
        }
        assert_eq!(ctx.scheduler.concurrency(), ctx.settings.get().max_vms);
    }
}

#[test]
fn general_agent_or_provider_switch_resets_an_unchanged_model() {
    use agentvm::domain::agents::{AgentKind, AgentSelection};
    use agentvm::domain::providers::Provider;
    let home = tempfile::tempdir().unwrap();
    let ctx = crate::helpers::ctx(home.path());
    let mut old = ctx.settings.get();
    old.model = Model::parse("opus").unwrap();
    ctx.update_settings(old).unwrap();
    let mut switched = ctx.settings.get();
    switched.agent = AgentSelection { kind: AgentKind::Qwen, provider: Some(Provider::Local), ..Default::default() };
    assert_eq!(ctx.update_settings(switched).unwrap().model.as_str(), "default");
    let mut custom = ctx.settings.get();
    custom.model = Model::parse("qwen-coder").unwrap();
    ctx.update_settings(custom).unwrap();
    let mut provider_switched = ctx.settings.get();
    provider_switched.agent.provider = Some(Provider::Openai);
    assert_eq!(ctx.update_settings(provider_switched).unwrap().model.as_str(), "default");
    let saved: Settings = serde_json::from_slice(&std::fs::read(home.path().join("settings.json")).unwrap()).unwrap();
    assert_eq!(saved.model.as_str(), "default");
}

#[test]
fn agent_switch_preserves_a_different_explicit_model() {
    use agentvm::domain::agents::{AgentKind, AgentSelection};
    use agentvm::domain::providers::Provider;
    let home = tempfile::tempdir().unwrap();
    let ctx = crate::helpers::ctx(home.path());
    let mut old = ctx.settings.get();
    old.model = Model::parse("opus").unwrap();
    ctx.update_settings(old).unwrap();
    let mut switched = ctx.settings.get();
    switched.agent = AgentSelection { kind: AgentKind::Qwen, provider: Some(Provider::Local), ..Default::default() };
    switched.model = Model::parse("qwen-coder").unwrap();
    assert_eq!(ctx.update_settings(switched).unwrap().model.as_str(), "qwen-coder");
}

#[test]
fn selected_provider_protocol_change_resets_the_saved_model() {
    use agentvm::app::agents::update_provider;
    use agentvm::domain::agents::{AgentKind, AgentSelection};
    use agentvm::domain::providers::{ApiProtocol, Provider, ProviderConfig};
    let home = tempfile::tempdir().unwrap();
    let ctx = crate::helpers::ctx(home.path());
    let config = ProviderConfig {
        endpoint: "http://127.0.0.1:1234/v1".into(),
        protocol: ApiProtocol::Openai,
        requires_key: false,
    };
    update_provider(&ctx, Provider::Custom, config.clone(), None).unwrap();
    let mut configured = ctx.settings.get();
    configured.agent = AgentSelection { kind: AgentKind::Qwen, provider: Some(Provider::Custom), ..Default::default() };
    configured.model = Model::parse("qwen-coder").unwrap();
    ctx.update_settings(configured).unwrap();
    update_provider(&ctx, Provider::Custom, ProviderConfig { protocol: ApiProtocol::Anthropic, ..config }, None)
        .unwrap();
    assert_eq!(ctx.settings.get().model.as_str(), "default");
}
