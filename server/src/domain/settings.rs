//! User settings, editable at runtime from the dashboard, and the host limits they are checked against.

use serde::{Deserialize, Serialize};

/// Memory left to macOS and the user's apps.
pub const HOST_RESERVE_MB: u64 = 8 * 1024;
pub const MIN_MEMORY_MB: u64 = 1024;
pub const MAX_VMS: usize = 64;

/// The Claude model for `claude --model`: `default` (the account's choice), an alias such as
/// `opus`, `sonnet[1m]` or `opusplan`, or a full model ID such as `claude-opus-5-5`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, type = "string"))]
pub struct Model(String);

impl Model {
    pub fn parse(s: &str) -> Result<Self, SettingsError> {
        let s = s.trim();
        let ok = !s.is_empty()
            && s.len() <= 64
            && s.starts_with(|c: char| c.is_ascii_lowercase())
            && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"-.[]_".contains(&c));
        if ok { Ok(Model(s.to_owned())) } else { Err(SettingsError::Model(s.to_owned())) }
    }

    pub fn default_choice() -> Self {
        Model("default".into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Value for `claude --model`, `None` to let Claude Code decide.
    pub fn cli_name(&self) -> Option<&str> {
        (self.0 != "default").then_some(self.0.as_str())
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::default_choice()
    }
}

impl TryFrom<String> for Model {
    type Error = SettingsError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<Model> for String {
    fn from(m: Model) -> String {
        m.0
    }
}

/// A Claude Code release: `latest`, `stable` or an exact version like `2.1.294`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, type = "string"))]
pub struct ClaudeVersion(String);

impl ClaudeVersion {
    pub fn parse(s: &str) -> Result<Self, SettingsError> {
        let s = s.trim();
        let semver = |v: &str| {
            let (core, pre) = v.split_once('-').unwrap_or((v, ""));
            let parts: Vec<&str> = core.split('.').collect();
            parts.len() == 3
                && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
                && pre.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.')
                && !(v.contains('-') && pre.is_empty())
        };
        if s == "latest" || s == "stable" || semver(s) {
            Ok(ClaudeVersion(s.to_owned()))
        } else {
            Err(SettingsError::ClaudeVersion(s.to_owned()))
        }
    }

    pub fn latest() -> Self {
        ClaudeVersion("latest".into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ClaudeVersion {
    fn default() -> Self {
        Self::latest()
    }
}

impl TryFrom<String> for ClaudeVersion {
    type Error = SettingsError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<ClaudeVersion> for String {
    fn from(v: ClaudeVersion) -> String {
        v.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Settings {
    /// VMs running at the same time; the others wait in the queue.
    pub max_vms: usize,
    /// vCPUs per VM.
    pub cpus: u32,
    /// Memory per VM.
    pub memory_mb: u64,
    /// Time limit for non-interactive tasks.
    pub timeout_s: u64,
    /// Claude model for new agents.
    pub model: Model,
    /// Repository suggested in the launcher.
    pub default_repo: Option<String>,
    /// Claude Code release installed when the VM image is (re)built.
    #[serde(default)]
    pub claude_version: ClaudeVersion,
    /// Where snapshots are backed up; the secret key is in the Keychain.
    #[serde(default)]
    pub s3: Option<super::s3::S3Config>,
    /// Snapshots of running terminals on a schedule (and before closing).
    #[serde(default)]
    pub auto_snapshots: super::snapshot::AutoSnapshots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct HostLimits {
    pub cpus: u32,
    pub ram_mb: u64,
}

impl HostLimits {
    /// How many VMs of `memory_mb` fit in RAM without pushing the Mac into swap.
    pub fn recommended_vms(&self, memory_mb: u64) -> usize {
        ((self.ram_mb.saturating_sub(HOST_RESERVE_MB)) / memory_mb.max(MIN_MEMORY_MB)).max(1) as usize
    }

    /// Resources for one VM must fit this Mac.
    pub fn check_vm(&self, cpus: u32, memory_mb: u64) -> Result<(), SettingsError> {
        if !(1..=self.cpus).contains(&cpus) {
            return Err(SettingsError::Cpus(self.cpus));
        }
        if !(MIN_MEMORY_MB..=self.max_memory_mb()).contains(&memory_mb) {
            return Err(SettingsError::Memory(self.max_memory_mb()));
        }
        Ok(())
    }

    pub fn max_memory_mb(&self) -> u64 {
        self.ram_mb.saturating_sub(HOST_RESERVE_MB).max(MIN_MEMORY_MB)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SettingsError {
    #[error("max VMs must be between 1 and {MAX_VMS}")]
    MaxVms,
    #[error("vCPUs per VM must be between 1 and {0} (this Mac's cores)")]
    Cpus(u32),
    #[error("memory per VM must be between {MIN_MEMORY_MB} and {0} MB")]
    Memory(u64),
    #[error("the task time limit must be between 60 seconds and 24 hours")]
    Timeout,
    #[error("unknown Claude Code version {0:?}: use latest, stable or a version like 2.1.294")]
    ClaudeVersion(String),
    #[error("invalid model {0:?}: use an alias like opus or sonnet[1m], or a model ID like claude-opus-5-5")]
    Model(String),
    #[error("S3: {0}")]
    S3(String),
    #[error(transparent)]
    AutoSnapshots(#[from] super::snapshot::AutoSnapshotsError),
}

impl Settings {
    pub fn validate(&self, host: &HostLimits) -> Result<(), SettingsError> {
        if !(1..=MAX_VMS).contains(&self.max_vms) {
            return Err(SettingsError::MaxVms);
        }
        host.check_vm(self.cpus, self.memory_mb)?;
        if !(60..=86_400).contains(&self.timeout_s) {
            return Err(SettingsError::Timeout);
        }
        if let Some(s3) = &self.s3 {
            s3.validate().map_err(|e| SettingsError::S3(e.to_string()))?;
        }
        self.auto_snapshots.validate()?;
        Ok(())
    }
}
