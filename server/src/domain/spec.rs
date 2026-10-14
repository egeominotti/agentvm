//! `task.json`: what the guest receives to run a task.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    pub prompt: String,
    pub branch: String,
    pub base_sha: String,
    pub timeout_s: u64,
    /// Interactive terminal: the VM stays on until the user closes it.
    #[serde(default)]
    pub interactive: bool,
    /// Coding CLI and its selected authentication provider.
    #[serde(default)]
    pub agent: crate::domain::agents::AgentSelection,
    /// Loopback relay to the host; no real provider credential enters the guest.
    #[serde(default)]
    pub agent_endpoint: Option<String>,
    /// Whether the frozen upstream accepts this CLI's native model aliases.
    #[serde(default)]
    pub agent_native_defaults: Option<bool>,
    #[serde(default)]
    pub agent_protocol: Option<crate::domain::providers::ApiProtocol>,
    /// An optional explicit API override; otherwise the CLI or provider chooses a model.
    #[serde(default)]
    pub model: Option<String>,
    /// Claude Code version to install at boot (`latest`, `stable` or `x.y.z`); absent keeps the image's.
    #[serde(default)]
    pub claude_version: Option<String>,
    /// The disk comes from a snapshot: keep `/root/work` and continue the last conversation.
    #[serde(default)]
    pub restore: bool,
}
