//! Reads the Claude token from the macOS Keychain (fallback: environment variable).

use std::path::PathBuf;
use std::process::Command;

use crate::secret::Secret;

const SERVICE: &str = "agentvm";
const ENV_FALLBACK: &str = "CLAUDE_CODE_OAUTH_TOKEN";

#[derive(Debug, thiserror::Error)]
pub enum KeychainError {
    #[error(
        "Claude token missing: generate one with `claude setup-token` and store it with \
         `security add-generic-password -s agentvm -a agentvm -w` (or set CLAUDE_CODE_OAUTH_TOKEN)"
    )]
    Missing,
}

pub struct Keychain {
    /// `None` = the user's default keychains.
    keychain: Option<PathBuf>,
}

impl Keychain {
    pub fn new(keychain: Option<PathBuf>) -> Self {
        Keychain { keychain }
    }

    pub fn read_token(&self) -> Result<Secret, KeychainError> {
        let mut cmd = Command::new("security");
        cmd.args(["find-generic-password", "-s", SERVICE, "-w"]);
        if let Some(kc) = &self.keychain {
            cmd.arg(kc);
        }
        if let Ok(out) = cmd.output() {
            let token = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if out.status.success() && !token.is_empty() {
                return Ok(Secret::new(token));
            }
        }
        match std::env::var(ENV_FALLBACK) {
            Ok(t) if !t.trim().is_empty() => Ok(Secret::new(t.trim().to_owned())),
            _ => Err(KeychainError::Missing),
        }
    }
}
