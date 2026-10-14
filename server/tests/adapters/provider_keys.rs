//! Provider keys are isolated in a real temporary macOS Keychain.
use crate::helpers::sh;
use agentvm::adapters::keychain::Keychain;
use agentvm::domain::providers::Provider;
use agentvm::secret::Secret;
use std::process::Command;

struct TempKeychain(std::path::PathBuf);
impl Drop for TempKeychain {
    fn drop(&mut self) {
        let _ = Command::new("chflags").arg("nouchg").arg(&self.0).output();
        let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
    }
}

#[test]
fn provider_keys_are_saved_replaced_and_deleted_independently() {
    let tmp = tempfile::tempdir().unwrap();
    let kc = TempKeychain(tmp.path().join("providers.keychain-db"));
    sh(tmp.path(), &format!("security create-keychain -p x {}", kc.0.display()));
    let keychain = Keychain::new(Some(kc.0.clone()));
    keychain.write_provider_key(Provider::Openai, &Secret::new("sk-openai-first".into())).unwrap();
    keychain.write_provider_key(Provider::Openai, &Secret::new("sk-openai-second".into())).unwrap();
    keychain.write_provider_key(Provider::Anthropic, &Secret::new("sk-anthropic".into())).unwrap();
    assert_eq!(keychain.read_provider_key(Provider::Openai).unwrap().expose(), "sk-openai-second");
    assert_eq!(keychain.read_provider_key(Provider::Anthropic).unwrap().expose(), "sk-anthropic");
    assert!(keychain.has_provider_key(Provider::Openai));
    keychain.delete_provider_key(Provider::Openai).unwrap();
    // Inspect the isolated Keychain directly, independent of host environment fallbacks.
    assert!(
        !Command::new("security")
            .args(["find-generic-password", "-s", "agentvm-provider-openai"])
            .arg(&kc.0)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(keychain.has_provider_key(Provider::Anthropic));
    keychain.delete_provider_key(Provider::Openai).unwrap();
    for bad in ["", "bad token", "bad\"token", "bad\\token", "bad\ntoken"] {
        assert!(keychain.write_provider_key(Provider::Google, &Secret::new(bad.into())).is_err());
    }
}

#[test]
fn provider_keys_use_standard_environment_fallbacks() {
    if std::env::var_os("AGENTVM_PROVIDER_ENV_TEST").is_some() {
        let tmp = tempfile::tempdir().unwrap();
        let keychain = Keychain::new(Some(tmp.path().join("absent.keychain-db")));
        assert_eq!(keychain.read_provider_key(Provider::Openai).unwrap().expose(), "env-openai");
        assert_eq!(keychain.read_provider_key(Provider::Google).unwrap().expose(), "env-gemini");
        assert_eq!(keychain.read_provider_key(Provider::Openrouter).unwrap().expose(), "env-openrouter");
        return;
    }
    // A child process keeps environment changes isolated from concurrent Keychain tests.
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "provider_keys::provider_keys_use_standard_environment_fallbacks"])
        .env("AGENTVM_PROVIDER_ENV_TEST", "1")
        .env("OPENAI_API_KEY", "env-openai")
        .env("GEMINI_API_KEY", "env-gemini")
        .env("OPENROUTER_API_KEY", "env-openrouter")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn provider_delete_reports_real_keychain_write_failures_without_losing_presence() {
    let tmp = tempfile::tempdir().unwrap();
    let kc = TempKeychain(tmp.path().join("immutable.keychain-db"));
    sh(tmp.path(), &format!("security create-keychain -p x {}", kc.0.display()));
    let keychain = Keychain::new(Some(kc.0.clone()));
    keychain.write_provider_key(Provider::Custom, &Secret::new("retained-key".into())).unwrap();
    assert!(Command::new("chflags").arg("uchg").arg(&kc.0).status().unwrap().success());
    let result = keychain.delete_provider_key(Provider::Custom);
    assert!(result.is_err(), "deletion reported success despite the immutable Keychain");
    assert!(keychain.has_provider_key(Provider::Custom));
    assert_eq!(keychain.read_provider_key(Provider::Custom).unwrap().expose(), "retained-key");
}
