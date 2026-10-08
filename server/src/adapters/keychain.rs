//! Lettura del token Claude dal Portachiavi di macOS (fallback: variabile d'ambiente).

use std::path::PathBuf;
use std::process::Command;

use crate::secret::Secret;

const SERVICE: &str = "agentvm";
const ENV_FALLBACK: &str = "CLAUDE_CODE_OAUTH_TOKEN";

#[derive(Debug, thiserror::Error)]
pub enum KeychainError {
    #[error(
        "token Claude assente: genera un token con `claude setup-token` e salvalo con \
         `security add-generic-password -s agentvm -a agentvm -w` (oppure imposta CLAUDE_CODE_OAUTH_TOKEN)"
    )]
    Missing,
}

pub struct Keychain {
    /// `None` = portachiavi predefiniti dell'utente.
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
