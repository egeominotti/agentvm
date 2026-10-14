//! Discoverable launch parameters; application validation remains authoritative.
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct LaunchArgs {
    /// Absolute host repository path or a git remote URL.
    pub repo_path: String,
    pub prompt: Option<String>,
    pub interactive: Option<bool>,
    pub base_ref: Option<String>,
    pub branch: Option<String>,
    pub model: Option<String>,
    pub agent: Option<AgentArgs>,
    pub cpus: Option<u32>,
    pub memory_mb: Option<u64>,
    pub tailscale: Option<bool>,
    pub claude_version: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct AgentArgs {
    pub kind: AgentKind,
    pub provider: Option<Provider>,
    pub version: Option<String>,
    pub custom: Option<CustomCli>,
    pub auth: Option<AgentAuth>,
    pub account_kind: Option<AccountKind>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Claude,
    Codex,
    Gemini,
    Opencode,
    Qwen,
    Copilot,
    Kilo,
    Aider,
    Goose,
    Custom,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Openai,
    Anthropic,
    Google,
    Openrouter,
    Alibabacodingplan,
    Opencodego,
    Opencodezen,
    Local,
    Custom,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "lowercase")]
pub enum AgentAuth {
    Account,
    Api,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Claude,
    Chatgpt,
    Google,
    Copilot,
    Kilo,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct CustomCli {
    pub name: String,
    /// Explicit installation script inside the guest VM.
    pub install: Option<String>,
    pub interactive: String,
    pub headless: Option<String>,
    pub resume: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::LaunchArgs;
    use serde_json::json;

    #[test]
    fn typed_launch_preserves_account_authentication_for_the_engine() {
        let input =
            json!({"repo_path":"/tmp/repository","agent":{"kind":"codex","auth":"account","account_kind":"chatgpt"}});
        let args: LaunchArgs = serde_json::from_value(input).unwrap();
        let body = serde_json::to_value(args).unwrap();
        assert_eq!(body["agent"]["auth"], "account");
        assert_eq!(body["agent"]["account_kind"], "chatgpt");
    }
}
