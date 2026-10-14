//! Agent defaults use real settings, records and host credential decisions.
use crate::helpers::{ctx, git_repo};
use agentvm::app::agents::{provider_config, read_credential};
use agentvm::app::store::Store;
use agentvm::app::submission::{NewTask, submit};
use agentvm::domain::agents::{AgentKind, AgentSelection};
use agentvm::domain::providers::Provider;
use agentvm::domain::snapshot::SnapshotId;

#[test]
fn custom_without_provider_and_local_provider_need_no_claude_token() {
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    let custom = AgentSelection { kind: AgentKind::Custom, ..Default::default() };
    assert!(provider_config(&ctx, &custom).is_none());
    assert_eq!(read_credential(&ctx, &custom).unwrap().expose(), "");
    let local = AgentSelection { kind: AgentKind::Codex, provider: Some(Provider::Local), ..Default::default() };
    assert_eq!(read_credential(&ctx, &local).unwrap().expose(), "");
}

#[tokio::test]
async fn non_claude_submission_uses_cli_default_and_persists_its_selection() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let ctx = ctx(home.path());
    let mut settings = ctx.settings.get();
    settings.model = agentvm::domain::settings::Model::parse("opus").unwrap();
    ctx.update_settings(settings).unwrap();
    let selected = AgentSelection {
        kind: AgentKind::Codex,
        provider: Some(Provider::Local),
        version: Some("1.2.3".into()),
        ..Default::default()
    };
    // Queued restore submission exercises defaults and persistence without launching a VM.
    // This current-thread runtime does not poll the launch task before it is dropped.
    let id = submit(
        &ctx,
        NewTask {
            repo: repo.path().to_str().unwrap(),
            prompt: "task".into(),
            base_ref: None,
            interactive: true,
            agent: Some(selected.clone()),
            model: None,
            claude_version: None,
            restore_from: Some(SnapshotId::parse("snap-20260101-000000-abcd").unwrap()),
            cpus: Some(1),
            memory_mb: Some(1024),
            label: None,
            tailscale: Some(false),
        },
    )
    .unwrap();
    let record = ctx.store.get(&id).unwrap();
    assert_eq!(record.model.as_str(), "default");
    assert_eq!(record.agent, selected);
    let records = Store::load(&home.path().join("jobs"));
    assert_eq!(records[0].agent, selected);
}

#[test]
fn old_task_records_restore_the_claude_selection() {
    let repo = git_repo();
    let record = crate::helpers::record(&repo);
    let mut value = serde_json::to_value(record).unwrap();
    value.as_object_mut().unwrap().remove("agent");
    let restored: agentvm::app::record::TaskRecord = serde_json::from_value(value).unwrap();
    assert_eq!(restored.agent.kind, AgentKind::Claude);
}

#[test]
fn agent_catalog_uses_configured_endpoints_and_protocols() {
    use agentvm::app::agents::update_provider;
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    for id in [Provider::Openai, Provider::Custom] {
        update_provider(&ctx, id, Provider::Alibabacodingplan.config(), None).unwrap();
    }
    update_provider(&ctx, Provider::Anthropic, Provider::Openai.config(), None).unwrap();
    update_provider(&ctx, Provider::Opencodego, Provider::Google.config(), None).unwrap();
    let catalog = agentvm::app::agents::catalog(&ctx);
    let providers = &catalog.agents.iter().find(|a| a.kind == AgentKind::Codex).unwrap().providers;
    for rejected in [Provider::Alibabacodingplan, Provider::Custom, Provider::Openai, Provider::Opencodego] {
        assert!(!providers.contains(&rejected), "Codex offered {rejected:?}");
    }
    assert!(providers.contains(&Provider::Anthropic));
    let gemini = &catalog.agents.iter().find(|a| a.kind == AgentKind::Gemini).unwrap().providers;
    assert!(gemini.contains(&Provider::Opencodego));
    let opencode = &catalog.agents.iter().find(|a| a.kind == AgentKind::Opencode).unwrap().providers;
    assert!(opencode.contains(&Provider::Alibabacodingplan));
}

#[test]
fn invalid_provider_changes_do_not_replace_credentials() {
    use agentvm::adapters::keychain::Keychain;
    use agentvm::app::agents::update_provider;
    use agentvm::domain::providers::{ApiProtocol, ProviderConfig};
    use agentvm::secret::Secret;
    use std::process::Command;
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
        }
    }
    let home = tempfile::tempdir().unwrap();
    let kc = Cleanup(home.path().join("invalid-provider.keychain-db"));
    assert!(Command::new("security").args(["create-keychain", "-p", "x"]).arg(&kc.0).status().unwrap().success());
    let mut ctx = ctx(home.path());
    std::sync::Arc::get_mut(&mut ctx).unwrap().keychain = Keychain::new(Some(kc.0.clone()));
    ctx.keychain.write_provider_key(Provider::Custom, &Secret::new("original-key".into())).unwrap();
    update_provider(
        &ctx,
        Provider::Custom,
        ProviderConfig { endpoint: "https://example.org/v1".into(), protocol: ApiProtocol::Openai, requires_key: true },
        None,
    )
    .unwrap();
    let mut settings = ctx.settings.get();
    settings.agent = AgentSelection { kind: AgentKind::Codex, provider: Some(Provider::Custom), ..Default::default() };
    ctx.update_settings(settings).unwrap();
    let result = update_provider(
        &ctx,
        Provider::Custom,
        ProviderConfig { endpoint: "https://example.org".into(), protocol: ApiProtocol::Anthropic, requires_key: true },
        Some("replacement-key".into()),
    );
    assert!(result.is_err());
    assert_eq!(ctx.keychain.read_provider_key(Provider::Custom).unwrap().expose(), "original-key");
}

#[test]
fn failed_provider_persistence_restores_the_actual_keychain_state() {
    use agentvm::adapters::keychain::Keychain;
    use agentvm::app::agents::update_provider;
    use agentvm::domain::providers::{ApiProtocol, ProviderConfig};
    use agentvm::secret::Secret;
    use std::process::Command;
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
        }
    }
    for previous in [Some("original-key"), None] {
        let home = tempfile::tempdir().unwrap();
        let kc = Cleanup(home.path().join("rollback.keychain-db"));
        assert!(Command::new("security").args(["create-keychain", "-p", "x"]).arg(&kc.0).status().unwrap().success());
        let mut ctx = ctx(home.path());
        std::sync::Arc::get_mut(&mut ctx).unwrap().keychain = Keychain::new(Some(kc.0.clone()));
        if let Some(previous) = previous {
            ctx.keychain.write_provider_key(Provider::Custom, &Secret::new(previous.into())).unwrap();
        }
        let old = ProviderConfig {
            endpoint: "https://old.example.org/v1".into(),
            protocol: ApiProtocol::Openai,
            requires_key: true,
        };
        update_provider(&ctx, Provider::Custom, old.clone(), None).unwrap();
        let path = home.path().join("settings.json");
        let backup = home.path().join("before.json");
        std::fs::rename(&path, &backup).unwrap();
        std::fs::create_dir(&path).unwrap();
        let result = update_provider(
            &ctx,
            Provider::Custom,
            ProviderConfig { endpoint: "https://new.example.org/v1".into(), ..old.clone() },
            Some("replacement-key".into()),
        );
        assert!(result.is_err());
        assert_eq!(ctx.settings.get().providers[&Provider::Custom], old);
        match previous {
            Some(previous) => assert_eq!(ctx.keychain.read_provider_key(Provider::Custom).unwrap().expose(), previous),
            None => assert!(
                !Command::new("security")
                    .args(["find-generic-password", "-s", "agentvm-provider-custom"])
                    .arg(&kc.0)
                    .output()
                    .unwrap()
                    .status
                    .success()
            ),
        }
        std::fs::remove_dir(&path).unwrap();
        std::fs::rename(&backup, &path).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["providers"]["custom"]["endpoint"], "https://old.example.org/v1");
    }
}

#[test]
fn failed_provider_compensation_is_reported_explicitly() {
    use agentvm::adapters::keychain::Keychain;
    use agentvm::domain::providers::{ApiProtocol, ProviderConfig};
    use agentvm::secret::Secret;
    use std::process::Command;
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = Command::new("chflags").arg("nouchg").arg(&self.0).output();
            let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
        }
    }
    let home = tempfile::tempdir().unwrap();
    let kc = Cleanup(home.path().join("compensation.keychain-db"));
    assert!(Command::new("security").args(["create-keychain", "-p", "x"]).arg(&kc.0).status().unwrap().success());
    let mut ctx = ctx(home.path());
    std::sync::Arc::get_mut(&mut ctx).unwrap().keychain = Keychain::new(Some(kc.0.clone()));
    ctx.keychain.write_provider_key(Provider::Custom, &Secret::new("original-key".into())).unwrap();
    std::fs::create_dir(home.path().join("settings.json")).unwrap();
    let result = ctx.settings.set_provider_with(
        Provider::Custom,
        ProviderConfig { endpoint: "https://example.org".into(), protocol: ApiProtocol::Openai, requires_key: true },
        || {
            let previous = ctx.keychain.read_stored_provider_key(Provider::Custom).map_err(|e| e.to_string())?.unwrap();
            ctx.keychain
                .write_provider_key(Provider::Custom, &Secret::new("replacement-key".into()))
                .map_err(|e| e.to_string())?;
            assert!(Command::new("chflags").arg("uchg").arg(&kc.0).status().unwrap().success());
            Ok(previous)
        },
        |previous| ctx.keychain.write_provider_key(Provider::Custom, &previous).map_err(|e| e.to_string()),
    );
    let error = result.unwrap_err().to_string();
    assert!(error.contains("rollback failed"), "{error}");
    assert!(error.contains("could not save"), "{error}");
}

#[test]
fn provider_rollback_restores_the_file_after_a_post_rename_sync_failure() {
    use agentvm::adapters::keychain::Keychain;
    use agentvm::app::agents::update_provider;
    use agentvm::domain::providers::{ApiProtocol, ProviderConfig};
    use agentvm::secret::Secret;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    struct Cleanup {
        home: std::path::PathBuf,
        kc: std::path::PathBuf,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.home, std::fs::Permissions::from_mode(0o700));
            let _ = Command::new("security").arg("delete-keychain").arg(&self.kc).output();
        }
    }
    let home = tempfile::tempdir().unwrap();
    let cleanup = Cleanup { home: home.path().into(), kc: home.path().join("post-rename.keychain-db") };
    assert!(Command::new("security").args(["create-keychain", "-p", "x"]).arg(&cleanup.kc).status().unwrap().success());
    let mut ctx = ctx(home.path());
    std::sync::Arc::get_mut(&mut ctx).unwrap().keychain = Keychain::new(Some(cleanup.kc.clone()));
    ctx.keychain.write_provider_key(Provider::Custom, &Secret::new("original-key".into())).unwrap();
    let old = ProviderConfig {
        endpoint: "https://old.example.org/v1".into(),
        protocol: ApiProtocol::Openai,
        requires_key: true,
    };
    update_provider(&ctx, Provider::Custom, old.clone(), None).unwrap();
    // Renames and temporary writes work; opening the directory to fsync after rename fails.
    std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o300)).unwrap();
    let result = update_provider(
        &ctx,
        Provider::Custom,
        ProviderConfig { endpoint: "https://new.example.org/v1".into(), ..old.clone() },
        Some("replacement-key".into()),
    );
    std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert_eq!(ctx.settings.get().providers[&Provider::Custom], old);
    assert_eq!(ctx.keychain.read_provider_key(Provider::Custom).unwrap().expose(), "original-key");
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.path().join("settings.json")).unwrap()).unwrap();
    assert_eq!(saved["providers"]["custom"]["endpoint"], "https://old.example.org/v1");
    assert!(result.unwrap_err().contains("configuration"));
}
