//! Claude's cost and token usage for one agent, as reported by Claude Code.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentUsage {
    /// What the session would cost at API prices (with a subscription it counts against its limits).
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub lines_added: u64,
    pub lines_removed: u64,
    /// Share of the context window in use.
    pub context_pct: Option<f64>,
    pub model: Option<String>,
}
