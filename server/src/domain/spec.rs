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
    /// Value for `claude --model`; absent to let Claude Code decide.
    #[serde(default)]
    pub model: Option<String>,
}
