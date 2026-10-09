//! The token: kept in a real Keychain, never printed.

use std::process::Command;

use agentvm::adapters::keychain::{Keychain, KeychainError};
use agentvm::secret::Secret;

use crate::helpers::sh;

struct TempKeychain(std::path::PathBuf);
impl Drop for TempKeychain {
    fn drop(&mut self) {
        let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
    }
}

#[test]
fn keychain_reads_token_from_a_real_keychain() {
    let tmp = tempfile::tempdir().unwrap();
    let kc = TempKeychain(tmp.path().join("t.keychain-db"));
    sh(tmp.path(), &format!("security create-keychain -p x {}", kc.0.display()));
    let keychain = Keychain::new(Some(kc.0.clone()));
    assert!(matches!(keychain.read_token(), Err(KeychainError::Missing)));
    sh(tmp.path(), &format!("security add-generic-password -s agentvm -a agentvm -w tok123 {}", kc.0.display()));
    assert_eq!(keychain.read_token().unwrap().expose(), "tok123");
}

/// Every open dashboard asks every few seconds whether a token is saved: answered from memory,
/// not by running `security` each time (~20 ms, and a secret read for nothing). A token saved
/// through agentvm counts at once.
#[test]
fn token_presence_is_answered_from_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let kc = TempKeychain(tmp.path().join("p.keychain-db"));
    sh(tmp.path(), &format!("security create-keychain -p x {}", kc.0.display()));
    let keychain = Keychain::new(Some(kc.0.clone()));
    let missing = keychain.token_status().unwrap_err();
    assert!(missing.contains("claude setup-token"), "{missing}");
    let t = std::time::Instant::now();
    for _ in 0..50 {
        assert!(keychain.token_status().is_err());
    }
    assert!(t.elapsed() < std::time::Duration::from_millis(100), "50 checks took {:?}", t.elapsed());
    keychain.write_token(&Secret::new("sk-ant-oat01-saved_Token-1".into())).unwrap();
    assert_eq!(keychain.token_status(), Ok(()));
    // Clones share what they know (the server hands its context to every request).
    assert_eq!(keychain.clone().token_status(), Ok(()));
}

/// The Tailscale auth key: one per Mac, in the Keychain like the other secrets.
#[test]
fn tailscale_key_is_kept_in_the_keychain() {
    let tmp = tempfile::tempdir().unwrap();
    let kc = TempKeychain(tmp.path().join("ts.keychain-db"));
    sh(tmp.path(), &format!("security create-keychain -p x {}", kc.0.display()));
    let keychain = Keychain::new(Some(kc.0.clone()));
    assert!(keychain.read_tailscale_key().is_none());
    keychain.write_tailscale_key(&Secret::new("tskey-auth-kAbC123CNTRL-0123456789abcdef".into())).unwrap();
    assert_eq!(keychain.read_tailscale_key().unwrap().expose(), "tskey-auth-kAbC123CNTRL-0123456789abcdef");
    keychain.write_tailscale_key(&Secret::new("tskey-client-kX-1?ephemeral=true&preauthorized=true".into())).unwrap();
    assert_eq!(keychain.read_tailscale_key().unwrap().expose(), "tskey-client-kX-1?ephemeral=true&preauthorized=true");
    keychain.delete_tailscale_key().unwrap();
    assert!(keychain.read_tailscale_key().is_none());
    keychain.delete_tailscale_key().unwrap();
    // It goes into a `security` command: nothing that could end the value early.
    assert!(keychain.write_tailscale_key(&Secret::new("tskey-auth-x\" -s evil".into())).is_err());
}

#[test]
fn missing_token_message_tells_how_to_fix() {
    assert!(KeychainError::Missing.to_string().contains("security add-generic-password -s agentvm -a agentvm -w"));
}

#[test]
fn keychain_token_can_be_written_and_read_back() {
    let tmp = tempfile::tempdir().unwrap();
    let kc = TempKeychain(tmp.path().join("w.keychain-db"));
    sh(tmp.path(), &format!("security create-keychain -p x {}", kc.0.display()));
    let keychain = Keychain::new(Some(kc.0.clone()));
    keychain.write_token(&Secret::new("sk-ant-oat01-first_Token-1".into())).unwrap();
    keychain.write_token(&Secret::new("sk-ant-oat01-second_Token-2".into())).unwrap();
    assert_eq!(keychain.read_token().unwrap().expose(), "sk-ant-oat01-second_Token-2");
    assert!(keychain.write_token(&Secret::new("bad token\" ; rm -rf /".into())).is_err());
}

#[test]
fn secret_redacts_its_own_value() {
    let secret = Secret::new("sk-ant-oat01-abc".into());
    assert_eq!(secret.redact("token=sk-ant-oat01-abc fine"), "token=[REDACTED] fine");
    assert_eq!(secret.redact("nothing to hide"), "nothing to hide");
}

/// Tokens for private repositories, one per git host, in the Keychain: written, listed by host
/// only, replaced and removed. A malformed token or host never reaches `security`.
#[test]
fn git_tokens_are_kept_per_host() {
    let tmp = tempfile::tempdir().unwrap();
    let kc = TempKeychain(tmp.path().join("g.keychain-db"));
    sh(tmp.path(), &format!("security create-keychain -p x {}", kc.0.display()));
    let keychain = Keychain::new(Some(kc.0.clone()));
    assert!(keychain.read_git_token("github.com").is_none());
    keychain.write_git_token("github.com", &Secret::new("github_pat_11AAAA_first".into())).unwrap();
    keychain.write_git_token("github.com", &Secret::new("github_pat_11AAAA_second".into())).unwrap();
    keychain.write_git_token("gitlab.com", &Secret::new("glpat-xyz".into())).unwrap();
    assert_eq!(keychain.read_git_token("github.com").unwrap().expose(), "github_pat_11AAAA_second");
    assert_eq!(
        keychain.git_token_hosts(&["github.com", "gitlab.com", "example.org"]),
        vec!["github.com", "gitlab.com"]
    );
    keychain.delete_git_token("github.com").unwrap();
    assert!(keychain.read_git_token("github.com").is_none());
    assert!(keychain.write_git_token("github.com", &Secret::new("bad token\" ; rm -rf /".into())).is_err());
    assert!(keychain.write_git_token("evil host -s x", &Secret::new("tok".into())).is_err());
}
