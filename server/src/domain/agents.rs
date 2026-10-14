//! Coding CLI registry and validated launch selections.
use super::accounts::{AccountKind, AgentAuth};
use super::custom_cli::CustomCli;
use super::providers::{ApiProtocol, Provider, ProviderConfig};
use super::settings::{ClaudeVersion, Model};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum AgentKind {
    #[default]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AgentSelection {
    pub kind: AgentKind,
    pub provider: Option<Provider>,
    pub version: Option<String>,
    pub custom: Option<CustomCli>,
    pub auth: Option<AgentAuth>,
    pub account_kind: Option<AccountKind>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AgentDescriptor {
    pub kind: AgentKind,
    pub name: String,
    pub providers: Vec<Provider>,
    pub headless: bool,
    pub resume: bool,
    pub requires_model: bool,
    pub accounts: Vec<AccountKind>,
}

impl AgentKind {
    pub fn accounts(self) -> &'static [AccountKind] {
        use AccountKind::*;
        match self {
            Self::Claude => &[Claude],
            Self::Codex | Self::Opencode => &[Chatgpt],
            Self::Gemini => &[Google],
            Self::Copilot => &[Copilot],
            Self::Kilo => &[Kilo, Chatgpt],
            Self::Goose => &[Claude, Chatgpt],
            _ => &[],
        }
    }
    pub const ALL: [Self; 10] = [
        Self::Claude,
        Self::Codex,
        Self::Gemini,
        Self::Opencode,
        Self::Qwen,
        Self::Copilot,
        Self::Kilo,
        Self::Aider,
        Self::Goose,
        Self::Custom,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Gemini => "Gemini CLI",
            Self::Opencode => "OpenCode",
            Self::Qwen => "Qwen Code",
            Self::Copilot => "GitHub Copilot CLI",
            Self::Kilo => "Kilo CLI",
            Self::Aider => "Aider",
            Self::Goose => "Goose",
            Self::Custom => "Custom CLI",
        }
    }
    pub fn default_provider(self) -> Option<Provider> {
        match self {
            Self::Claude | Self::Custom => None,
            Self::Gemini => Some(Provider::Google),
            _ => Some(Provider::Openai),
        }
    }
    pub fn supports(self, protocol: ApiProtocol) -> bool {
        match self {
            Self::Claude => protocol == ApiProtocol::Anthropic,
            Self::Codex | Self::Goose => protocol == ApiProtocol::Openai,
            Self::Qwen | Self::Copilot => matches!(protocol, ApiProtocol::Openai | ApiProtocol::Anthropic),
            Self::Gemini => protocol == ApiProtocol::Google,
            _ => true,
        }
    }
    pub fn supports_config(self, config: &ProviderConfig) -> bool {
        // Codex 0.162.1 requires Responses; Coding Plan supports only Chat Completions and Messages.
        self.supports(config.protocol)
            && !(self == Self::Codex && super::provider_models::is_alibaba_coding_plan(config))
    }
    pub fn requires_model(self) -> bool {
        matches!(self, Self::Opencode | Self::Qwen | Self::Copilot | Self::Kilo | Self::Aider | Self::Goose)
    }
    pub fn descriptor(self) -> AgentDescriptor {
        AgentDescriptor {
            kind: self,
            name: self.name().into(),
            providers: Provider::ALL
                .into_iter()
                .filter(|p| *p == Provider::Custom || self.supports_config(&p.config()))
                .collect(),
            headless: true,
            resume: true,
            requires_model: self.requires_model(),
            accounts: self.accounts().to_vec(),
        }
    }
}

impl AgentSelection {
    pub fn resolved_account(&self) -> Option<AccountKind> {
        if self.auth == Some(AgentAuth::Api) {
            return None;
        }
        if let Some(account) = self.account_kind {
            return Some(account);
        }
        if self.auth == Some(AgentAuth::Account) {
            return self.kind.accounts().first().copied();
        }
        (self.kind == AgentKind::Claude && self.provider.is_none()).then_some(AccountKind::Claude)
    }
    pub fn resolved_provider(&self) -> Option<Provider> {
        if self.resolved_account().is_some() {
            return None;
        }
        if self.kind == AgentKind::Claude && self.auth == Some(AgentAuth::Api) {
            return Some(self.provider.unwrap_or(Provider::Anthropic));
        }
        self.provider.or(self.kind.default_provider())
    }
    pub fn validate(&self, config: Option<&ProviderConfig>, interactive: bool) -> Result<(), String> {
        if self.auth == Some(AgentAuth::Api) && self.account_kind.is_some() {
            return Err("API authentication cannot select an account".into());
        }
        if self.auth == Some(AgentAuth::Account) || self.account_kind.is_some() {
            let account = self.resolved_account().ok_or("this CLI has no supported account authentication")?;
            if !self.kind.accounts().contains(&account) {
                return Err("this CLI does not support the selected account".into());
            }
            if self.provider.is_some() {
                return Err("account authentication cannot select an API provider".into());
            }
        }
        if let Some(v) = &self.version {
            ClaudeVersion::parse(v).map_err(|e| e.to_string())?;
            if self.kind != AgentKind::Claude && v == "stable" {
                return Err("stable is only a Claude Code version channel".into());
            }
        }
        if let Some(config) = config {
            config.validate()?;
            if self.resolved_account().is_none() && !self.kind.supports_config(config) {
                return Err(format!(
                    "{} does not support the selected provider endpoint or protocol",
                    self.kind.name()
                ));
            }
        }
        if self.kind == AgentKind::Custom {
            self.custom.as_ref().ok_or("custom CLI needs command templates")?.validate(interactive)?;
        } else if self.custom.is_some() {
            return Err("custom commands require the custom CLI selection".into());
        }
        Ok(())
    }
    /// Explicit IDs are validated by Model; omitted IDs are chosen by the guest at startup.
    pub fn validate_model(&self, _model: &Model) -> Result<(), String> {
        Ok(())
    }
}
