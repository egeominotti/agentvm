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
    #[error("that does not look like a Claude token (expected sk-ant-…)")]
    InvalidToken,
    #[error("could not save the token in the Keychain: {0}")]
    WriteFailed(String),
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

    /// Saves (or replaces) the token. It goes to `security` on stdin, never on the command line.
    pub fn write_token(&self, token: &Secret) -> Result<(), KeychainError> {
        let t = token.expose();
        let well_formed = t.starts_with("sk-ant-") && t.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_');
        if !well_formed {
            return Err(KeychainError::InvalidToken);
        }
        let mut line = format!("add-generic-password -U -s {SERVICE} -a {SERVICE} -w {t}");
        if let Some(kc) = &self.keychain {
            line.push_str(&format!(" \"{}\"", kc.display()));
        }
        line.push('\n');
        let mut child = Command::new("security")
            .arg("-i")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| KeychainError::WriteFailed(e.to_string()))?;
        {
            use std::io::Write;
            let mut stdin = child.stdin.take().expect("stdin is piped");
            stdin.write_all(line.as_bytes()).map_err(|e| KeychainError::WriteFailed(e.to_string()))?;
        }
        let out = child.wait_with_output().map_err(|e| KeychainError::WriteFailed(e.to_string()))?;
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        if out.status.success() && stderr.is_empty() { Ok(()) } else { Err(KeychainError::WriteFailed(stderr)) }
    }
}
