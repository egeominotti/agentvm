//! User settings, editable at runtime from the dashboard, and the host limits they are checked against.

use serde::{Deserialize, Serialize};

/// Memory left to macOS and the user's apps.
pub const HOST_RESERVE_MB: u64 = 8 * 1024;
pub const MIN_MEMORY_MB: u64 = 1024;
pub const MAX_VMS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Model {
    /// Whatever Claude Code picks for the account.
    Default,
    Sonnet,
    Opus,
    Haiku,
}

impl Model {
    /// Value for `claude --model`, `None` to let Claude Code decide.
    pub fn cli_name(self) -> Option<&'static str> {
        match self {
            Model::Default => None,
            Model::Sonnet => Some("sonnet"),
            Model::Opus => Some("opus"),
            Model::Haiku => Some("haiku"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HostLimits {
    pub cpus: u32,
    pub ram_mb: u64,
}

impl HostLimits {
    /// How many VMs of `memory_mb` fit in RAM without pushing the Mac into swap.
    pub fn recommended_vms(&self, memory_mb: u64) -> usize {
        ((self.ram_mb.saturating_sub(HOST_RESERVE_MB)) / memory_mb.max(MIN_MEMORY_MB)).max(1) as usize
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
}

impl Settings {
    pub fn validate(&self, host: &HostLimits) -> Result<(), SettingsError> {
        if !(1..=MAX_VMS).contains(&self.max_vms) {
            return Err(SettingsError::MaxVms);
        }
        if !(1..=host.cpus).contains(&self.cpus) {
            return Err(SettingsError::Cpus(host.cpus));
        }
        if !(MIN_MEMORY_MB..=host.max_memory_mb()).contains(&self.memory_mb) {
            return Err(SettingsError::Memory(host.max_memory_mb()));
        }
        if !(60..=86_400).contains(&self.timeout_s) {
            return Err(SettingsError::Timeout);
        }
        Ok(())
    }
}
