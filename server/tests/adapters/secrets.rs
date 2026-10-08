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
