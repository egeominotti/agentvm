//! Inference providers and non-secret endpoint configuration.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum ApiProtocol {
    Openai,
    Anthropic,
    Google,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ProviderConfig {
    pub endpoint: String,
    pub protocol: ApiProtocol,
    pub requires_key: bool,
}

impl Provider {
    pub const ALL: [Self; 9] = [
        Self::Openai,
        Self::Anthropic,
        Self::Google,
        Self::Openrouter,
        Self::Alibabacodingplan,
        Self::Opencodego,
        Self::Opencodezen,
        Self::Local,
        Self::Custom,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Google => "google",
            Self::Openrouter => "openrouter",
            Self::Alibabacodingplan => "alibabacodingplan",
            Self::Opencodego => "opencodego",
            Self::Opencodezen => "opencodezen",
            Self::Local => "local",
            Self::Custom => "custom",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Openai => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Google => "Google Gemini",
            Self::Openrouter => "OpenRouter",
            Self::Alibabacodingplan => "Alibaba Coding Plan (subscription key)",
            Self::Opencodego => "OpenCode Go / Go Plus (subscription key)",
            Self::Opencodezen => "OpenCode Zen (usage billing)",
            Self::Local => "Local",
            Self::Custom => "Custom",
        }
    }
    pub fn config(self) -> ProviderConfig {
        let (endpoint, protocol, requires_key) = match self {
            Self::Openai => ("https://api.openai.com/v1", ApiProtocol::Openai, true),
            Self::Anthropic => ("https://api.anthropic.com", ApiProtocol::Anthropic, true),
            Self::Google => ("https://generativelanguage.googleapis.com", ApiProtocol::Google, true),
            Self::Openrouter => ("https://openrouter.ai/api/v1", ApiProtocol::Openai, true),
            Self::Alibabacodingplan => ("https://coding-intl.dashscope.aliyuncs.com/v1", ApiProtocol::Openai, true),
            Self::Opencodego => ("https://opencode.ai/inference/go/v1", ApiProtocol::Openai, true),
            Self::Opencodezen => ("https://opencode.ai/inference/v1", ApiProtocol::Openai, true),
            Self::Local => ("http://127.0.0.1:11434/v1", ApiProtocol::Openai, false),
            Self::Custom => ("", ApiProtocol::Openai, true),
        };
        ProviderConfig { endpoint: endpoint.into(), protocol, requires_key }
    }
}

impl ProviderConfig {
    pub fn validate(&self) -> Result<(), String> {
        let url = url::Url::parse(&self.endpoint).map_err(|_| "provider endpoint must be an absolute HTTP URL")?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("provider endpoint must be HTTP(S), without credentials, query or fragment".into());
        }
        Ok(())
    }
}
