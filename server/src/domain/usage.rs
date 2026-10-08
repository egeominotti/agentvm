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

/// Claude's usage at one moment of the VM's life (host clock, seconds since the epoch).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UsageSample {
    pub at: f64,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub context_pct: Option<f64>,
}

impl UsageSample {
    pub fn of(u: &AgentUsage, at: f64) -> Self {
        UsageSample {
            at,
            cost_usd: u.cost_usd,
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            context_pct: u.context_pct,
        }
    }

    /// The same usage, whenever it was taken.
    pub fn same_as(&self, other: &UsageSample) -> bool {
        UsageSample { at: 0.0, ..self.clone() } == UsageSample { at: 0.0, ..other.clone() }
    }
}
