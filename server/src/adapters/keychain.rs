//! Reads the Claude token from the macOS Keychain (fallback: environment variable).

use std::path::PathBuf;
use std::process::Command;

use crate::secret::Secret;

/// A locked Keychain may wait for an unlock dialog nobody answers: never block on it forever.
const KEYCHAIN_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);

const SERVICE: &str = "agentvm";
const S3_SERVICE: &str = "agentvm-s3";
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
    #[error("no S3 secret key saved yet")]
    MissingS3Secret,
    #[error("the S3 secret key contains unsupported characters")]
    InvalidS3Secret,
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
        if let Ok(out) = crate::process::output(&mut cmd, KEYCHAIN_LIMIT) {
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
        let well_formed =
            t.starts_with("sk-ant-") && t.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_');
        if !well_formed {
            return Err(KeychainError::InvalidToken);
        }
        self.write(SERVICE, t)
    }

    /// `AGENTVM_S3_SECRET` wins over the Keychain (handy for CI and tests).
    pub fn read_s3_secret(&self) -> Result<Secret, KeychainError> {
        if let Ok(s) = std::env::var("AGENTVM_S3_SECRET")
            && !s.trim().is_empty()
        {
            return Ok(Secret::new(s.trim().to_owned()));
        }
        self.read(S3_SERVICE).ok_or(KeychainError::MissingS3Secret)
    }

    pub fn write_s3_secret(&self, secret: &Secret) -> Result<(), KeychainError> {
        let s = secret.expose();
        if s.is_empty() || !s.bytes().all(|c| c.is_ascii_graphic() && c != b'"' && c != b'\\') {
            return Err(KeychainError::InvalidS3Secret);
        }
        self.write(S3_SERVICE, &format!("\"{s}\""))
    }

    fn read(&self, service: &str) -> Option<Secret> {
        let mut cmd = Command::new("security");
        cmd.args(["find-generic-password", "-s", service, "-w"]);
        if let Some(kc) = &self.keychain {
            cmd.arg(kc);
        }
        let out = crate::process::output(&mut cmd, KEYCHAIN_LIMIT).ok()?;
        let value = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        (out.status.success() && !value.is_empty()).then(|| Secret::new(value))
    }

    /// `value` is already quoted when needed; it reaches `security` on stdin.
    fn write(&self, service: &str, value: &str) -> Result<(), KeychainError> {
        let mut line = format!("add-generic-password -U -s {service} -a {service} -w {value}");
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
        let out = crate::process::wait_output(child, KEYCHAIN_LIMIT, "security")
            .map_err(|e| KeychainError::WriteFailed(e.to_string()))?;
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        if out.status.success() && stderr.is_empty() { Ok(()) } else { Err(KeychainError::WriteFailed(stderr)) }
    }
}
