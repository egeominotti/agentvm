//! Reads the Claude token from the macOS Keychain (fallback: environment variable).

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::secret::Secret;

/// A locked Keychain may wait for an unlock dialog nobody answers: never block on it forever.
const KEYCHAIN_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);

const SERVICE: &str = "agentvm";
const S3_SERVICE: &str = "agentvm-s3";
const TAILSCALE_SERVICE: &str = "agentvm-tailscale";
const ENV_FALLBACK: &str = "CLAUDE_CODE_OAUTH_TOKEN";
/// How long the answer to "is a token saved?" is trusted. Tokens saved through agentvm count at
/// once; one added or removed with `security` by hand shows within this.
const PRESENCE_TTL: Duration = Duration::from_secs(30);

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
    #[error("that is not a git token for a host (one line, no quotes or spaces)")]
    InvalidGitToken,
    #[error("that is not a Tailscale auth key (expected tskey-auth-… or tskey-client-…)")]
    InvalidTailscaleKey,
}

/// Whether a token is saved (or why not), and when that was read.
type Presence = Option<(Instant, Result<(), String>)>;

#[derive(Clone)]
pub struct Keychain {
    /// `None` = the user's default keychains.
    keychain: Option<PathBuf>,
    /// Whether a token is saved, and when that was last read (shared by clones).
    presence: Arc<Mutex<Presence>>,
}

impl Keychain {
    pub fn new(keychain: Option<PathBuf>) -> Self {
        Keychain { keychain, presence: Arc::default() }
    }

    /// Whether a token is saved (or why not), without the token itself. Every open dashboard asks
    /// every few seconds: answered from memory, `security` runs at most once per `PRESENCE_TTL`.
    pub fn token_status(&self) -> Result<(), String> {
        let mut known = self.presence.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, status)) = known.as_ref()
            && at.elapsed() < PRESENCE_TTL
        {
            return status.clone();
        }
        let status = self.read_token().map(drop).map_err(|e| e.to_string());
        *known = Some((Instant::now(), status.clone()));
        status
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
        self.write(SERVICE, t)?;
        *self.presence.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), Ok(())));
        Ok(())
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

    /// The token for private repositories on `host` (github.com…), if one was saved.
    pub fn read_git_token(&self, host: &str) -> Option<Secret> {
        valid_host(host).then(|| self.read(&git_service(host))).flatten()
    }

    /// Saves (or replaces) the token for `host`; on stdin to `security`, never on a command line.
    pub fn write_git_token(&self, host: &str, token: &Secret) -> Result<(), KeychainError> {
        let t = token.expose().trim();
        let well_formed =
            !t.is_empty() && t.len() <= 512 && t.bytes().all(|c| c.is_ascii_graphic() && c != b'"' && c != b'\\');
        if !valid_host(host) || !well_formed {
            return Err(KeychainError::InvalidGitToken);
        }
        self.write(&git_service(host), &format!("\"{t}\""))
    }

    pub fn delete_git_token(&self, host: &str) -> Result<(), KeychainError> {
        if !valid_host(host) {
            return Err(KeychainError::InvalidGitToken);
        }
        self.delete(&git_service(host))
    }

    /// The key VMs join the user's tailnet with, if one was saved.
    pub fn read_tailscale_key(&self) -> Option<Secret> {
        self.read(TAILSCALE_SERVICE)
    }

    /// Saves (or replaces) the Tailscale auth key; on stdin to `security`, never on a command line.
    pub fn write_tailscale_key(&self, key: &Secret) -> Result<(), KeychainError> {
        let k = key.expose().trim();
        if !crate::domain::tailscale::valid_auth_key(k) {
            return Err(KeychainError::InvalidTailscaleKey);
        }
        self.write(TAILSCALE_SERVICE, &format!("\"{k}\""))
    }

    pub fn delete_tailscale_key(&self) -> Result<(), KeychainError> {
        self.delete(TAILSCALE_SERVICE)
    }

    /// Which of `hosts` have a token saved (the tokens themselves are never listed).
    pub fn git_token_hosts<'h>(&self, hosts: &[&'h str]) -> Vec<&'h str> {
        hosts.iter().copied().filter(|h| self.read_git_token(h).is_some()).collect()
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

    /// Already absent is fine: the secret is gone either way.
    fn delete(&self, service: &str) -> Result<(), KeychainError> {
        let mut cmd = Command::new("security");
        cmd.args(["delete-generic-password", "-s", service]);
        if let Some(kc) = &self.keychain {
            cmd.arg(kc);
        }
        crate::process::output(&mut cmd, KEYCHAIN_LIMIT).map_err(|e| KeychainError::WriteFailed(e.to_string()))?;
        Ok(())
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

fn git_service(host: &str) -> String {
    format!("agentvm-git-{host}")
}

/// A host name only: it becomes part of a `security` command line.
fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.contains('.')
        && host.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-')
}
