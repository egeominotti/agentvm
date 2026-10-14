//! Account sessions are encoded before being passed to security's stdin interpreter.
use super::{KEYCHAIN_LIMIT, Keychain, KeychainError, missing_item};
use crate::domain::accounts::AccountKind;
use crate::secret::Secret;
use base64::Engine;

fn service(account: AccountKind) -> String {
    format!("agentvm-account-{}", account.id())
}
impl Keychain {
    pub fn delete_claude_account(&self) -> Result<(), KeychainError> {
        self.delete(super::SERVICE)?;
        *self.presence.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }
    pub fn read_account(&self, account: AccountKind) -> Result<Option<Secret>, KeychainError> {
        self.read_checked(&service(account))?
            .map(|s| {
                base64::engine::general_purpose::STANDARD
                    .decode(s.expose())
                    .ok()
                    .and_then(|b| String::from_utf8(b).ok())
                    .map(Secret::new)
                    .ok_or_else(|| KeychainError::ReadFailed("account session is malformed".into()))
            })
            .transpose()
    }
    pub fn write_account(&self, account: AccountKind, session: &Secret) -> Result<(), KeychainError> {
        let value = base64::engine::general_purpose::STANDARD.encode(session.expose());
        self.write(&service(account), &value)
    }
    pub fn delete_account(&self, account: AccountKind) -> Result<(), KeychainError> {
        self.delete(&service(account))
    }
    /// Only documented native CLI services can be referenced; credentials are never displayed.
    pub fn read_cli_account(&self, service: &str, account: Option<&str>) -> Result<Option<Secret>, KeychainError> {
        if !matches!(service, "Codex Auth" | "gemini-cli-oauth" | "copilot-cli") {
            return Err(KeychainError::ReadFailed("unknown native account service".into()));
        }
        let mut command = std::process::Command::new("security");
        command.args(["find-generic-password", "-s", service, "-w"]);
        if let Some(account) = account {
            command.args(["-a", account]);
        }
        if let Some(path) = &self.keychain {
            command.arg(path);
        }
        let out = crate::process::output(&mut command, KEYCHAIN_LIMIT)
            .map_err(|_| KeychainError::ReadFailed("native account Keychain unavailable".into()))?;
        if out.status.success() {
            return Ok(Some(Secret::new(String::from_utf8_lossy(&out.stdout).trim().into())));
        }
        if missing_item(&out) {
            return Ok(None);
        }
        Err(KeychainError::ReadFailed("native account Keychain unavailable".into()))
    }
}
