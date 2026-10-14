//! Model catalogs for subscription endpoints with a fixed list or several request formats.
use super::agents::AgentKind;
use super::providers::{ApiProtocol, ProviderConfig};
use serde_json::{Value, json};
use url::Url;

pub fn is_alibaba_coding_plan(config: &ProviderConfig) -> bool {
    Url::parse(&config.endpoint).is_ok_and(|url| {
        url.scheme() == "https"
            && matches!(url.host_str(), Some("coding.dashscope.aliyuncs.com" | "coding-intl.dashscope.aliyuncs.com"))
            && matches!(url.path().trim_end_matches('/'), "/v1" | "/apps/anthropic" | "/apps/anthropic/v1")
    })
}

pub fn preset_catalog(config: &ProviderConfig) -> Option<Value> {
    let url = Url::parse(&config.endpoint).ok()?;
    if !is_alibaba_coding_plan(config) {
        return None;
    }
    // Model Studio has no /models API; use a currently supported Coding Plan model.
    // https://www.alibabacloud.com/help/en/model-studio/coding-plan
    match (config.protocol, url.path().trim_end_matches('/')) {
        (ApiProtocol::Openai, "/v1") => Some(json!({"object":"list","data":[{"id":"qwen3.7-plus","object":"model"}]})),
        (ApiProtocol::Anthropic, "/apps/anthropic" | "/apps/anthropic/v1") => Some(json!({
            "data":[{"id":"qwen3.7-plus","display_name":"Qwen3.7 Plus","type":"model"}],
            "has_more":false
        })),
        _ => None,
    }
}

pub fn is_opencode(config: &ProviderConfig) -> bool {
    opencode_go(config).is_some()
}

fn opencode_go(config: &ProviderConfig) -> Option<bool> {
    let url = Url::parse(&config.endpoint).ok()?;
    if url.scheme() != "https" || url.host_str() != Some("opencode.ai") {
        return None;
    }
    match url.path().trim_end_matches('/') {
        "/inference/v1" | "/zen/v1" => Some(false),
        "/inference/go/v1" | "/zen/go/v1" => Some(true),
        _ => None,
    }
}

pub fn filter_catalog(config: &ProviderConfig, kind: AgentKind, mut payload: Value) -> Option<Value> {
    let go = opencode_go(config)?;
    let models = payload.get_mut("data")?.as_array_mut()?;
    let format = match config.protocol {
        ApiProtocol::Anthropic => RequestFormat::Messages,
        ApiProtocol::Google => RequestFormat::Google,
        ApiProtocol::Openai if kind == AgentKind::Codex => RequestFormat::Responses,
        ApiProtocol::Openai => RequestFormat::Chat,
    };
    models.retain(|model| {
        let id = model.get("id").and_then(Value::as_str).unwrap_or_default();
        opencode_format(go, id) == Some(format)
    });
    Some(payload)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestFormat {
    Chat,
    Responses,
    Messages,
    Google,
}

fn opencode_format(go: bool, id: &str) -> Option<RequestFormat> {
    use RequestFormat::*;
    // The catalog has no request-format metadata. These are the documented endpoint tables:
    // https://opencode.ai/v2/docs/console/go/#endpoints
    // https://opencode.ai/v2/docs/console/models/
    // Unknown IDs remain unavailable until their format is documented.
    match id {
        "gpt-6-luna" | "gpt-5.6-luna" | "grok-4.7" | "grok-4.6" => Some(Responses),
        "claude-haiku-5-5" | "qwen3.8-flash" | "qwen3.7-plus" => Some(Messages),
        "qwen3.8-max" | "minimax-m3" | "minimax-m2.7" => Some(if go { Messages } else { Chat }),
        "glm-5.3-flash"
        | "glm-5.3"
        | "glm-5.2"
        | "kimi-k3"
        | "kimi-k2.7-code"
        | "kimi-k2.6"
        | "deepseek-v4.1-flash"
        | "deepseek-v4-pro"
        | "longcat-2.5-preview-free"
        | "step-5-preview-free"
        | "cat-jam-free" => Some(Chat),
        _ if go => match id {
            "muse-spark-1.3-contributor" | "muse-spark-1.2-contributor" => Some(Responses),
            "gemini-4-argon-contributor" => Some(Google),
            "longcat-2.0" | "mimo-v2.6-flash" | "mimo-v2.6-pro" | "mimo-v2.5" | "mimo-v2.5-pro" | "hy4-preview"
            | "hy3" | "space-bunny" => Some(Chat),
            _ => None,
        },
        _ => match id {
            "gpt-6-astra"
            | "gpt-6-sol"
            | "gpt-6.1-sol"
            | "gpt-5.6-sol"
            | "gpt-5.6-terra"
            | "gpt-5.5"
            | "gpt-5.5-pro"
            | "gpt-5.4"
            | "gpt-5.4-pro"
            | "gpt-5.4-mini"
            | "gpt-5.4-nano"
            | "gpt-5.3-codex"
            | "gpt-5.3-codex-spark"
            | "gpt-5.2"
            | "gpt-5.2-codex"
            | "gpt-5.1"
            | "gpt-5.1-codex"
            | "gpt-5.1-codex-max"
            | "gpt-5.1-codex-mini"
            | "gpt-5"
            | "gpt-5-codex"
            | "gpt-5-nano"
            | "grok-4.5"
            | "grok-build-0.1"
            | "muse-spark-1.3"
            | "muse-spark-1.2"
            | "muse-spark-1.3-contributor-free" => Some(Responses),
            "claude-fable-5-1" | "claude-fable-5" | "claude-opus-5-5" | "claude-opus-5" | "claude-opus-4-8"
            | "claude-opus-4-7" | "claude-opus-4-6" | "claude-opus-4-5" | "claude-sonnet-5-5" | "claude-sonnet-5"
            | "claude-sonnet-4-6" | "claude-sonnet-4-5" | "claude-haiku-4-5" | "qwen3.7-max" | "qwen3.6-plus"
            | "qwen3.5-plus" => Some(Messages),
            "gemini-4-argon"
            | "gemini-3.8-flash"
            | "gemini-3.7-flash"
            | "gemini-3.6-flash"
            | "gemini-3.5-flash"
            | "gemini-3.5-flash-lite"
            | "gemini-3.1-pro"
            | "gemini-3-flash" => Some(Google),
            "minimax-m2.5"
            | "glm-5.1"
            | "glm-5"
            | "kimi-k2.5"
            | "mistral-large-4"
            | "big-pickle"
            | "space-bunny-free"
            | "exo-free"
            | "mimo-v2.6-flash-free"
            | "mimo-v2.5-free"
            | "nemotron-3-ultra-free"
            | "nemotron-3.5-lightning-free" => Some(Chat),
            _ => None,
        },
    }
}
