//! Native account relay targets and Responses request compatibility.
use super::accounts::AccountKind;
use super::providers::ProviderConfig;
use serde_json::Value;
use url::Url;

pub fn account_target(account: AccountKind, config: &ProviderConfig, path: &str) -> Result<Url, String> {
    config.validate()?;
    let endpoint = Url::parse(&config.endpoint).map_err(|_| "invalid account endpoint")?;
    let host = endpoint.host_str().unwrap_or_default();
    let base = endpoint.path().trim_end_matches('/');
    let verified = endpoint.scheme() == "https"
        && endpoint.port_or_known_default() == Some(443)
        && match account {
            AccountKind::Claude => host == "api.anthropic.com" && base.is_empty(),
            AccountKind::Chatgpt => host == "chatgpt.com" && base == "/backend-api/codex",
            AccountKind::Google => host == "cloudcode-pa.googleapis.com" && base.is_empty(),
            AccountKind::Copilot => host.ends_with(".githubcopilot.com") && base.is_empty(),
            AccountKind::Kilo => host == "api.kilo.ai" && base == "/api/openrouter",
        };
    if !verified {
        return Err("account relay requires its verified native service endpoint".into());
    }
    let url = super::agent_proxy::target_url(&config.endpoint, path)?;
    let path = url.path();
    let allowed = match account {
        AccountKind::Claude => path.starts_with("/v1/"),
        AccountKind::Chatgpt => matches!(path, "/backend-api/codex/models" | "/backend-api/codex/responses"),
        AccountKind::Google => path.starts_with("/v1internal:") || path.starts_with("/v1internal/"),
        AccountKind::Copilot => {
            !path.starts_with("/models/session")
                && (path == "/models"
                    || path.starts_with("/models/")
                    || matches!(
                        path,
                        "/responses" | "/chat/completions" | "/v1/messages" | "/v1/messages/count_tokens"
                    ))
        }
        AccountKind::Kilo => path.starts_with("/api/openrouter/"),
    };
    if !allowed {
        return Err("this account relay accepts inference and model discovery only".into());
    }
    Ok(url)
}

pub fn account_request_header(name: &str) -> bool {
    super::agent_proxy::request_header(name)
        || matches!(
            name,
            "copilot-integration-id"
                | "x-github-api-version"
                | "x-interaction-id"
                | "x-initiator"
                | "x-request-id"
                | "x-goog-user-project"
                | "originator"
                | "session_id"
                | "x-client-request-id"
                | "x-codex-beta-features"
                | "x-codex-turn-id"
                | "x-codex-installation-id"
        )
}

/// Copilot catalogs require the original GitHub OAuth token, never its inference token.
pub fn uses_catalog_credential(account: AccountKind, method: &str, path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path);
    account == AccountKind::Copilot
        && method == "GET"
        && !path.starts_with("/models/session")
        && (path == "/models" || path.starts_with("/models/"))
}

pub fn account_authorization(account: AccountKind, config: &ProviderConfig, key: &str, catalog: bool) -> String {
    let host = Url::parse(&config.endpoint).ok().and_then(|u| u.host_str().map(str::to_owned));
    let prefix = if account == AccountKind::Copilot
        && catalog
        && !key.starts_with("github_pat_")
        && matches!(host.as_deref(), Some("api.business.githubcopilot.com" | "api.enterprise.githubcopilot.com"))
    {
        "token"
    } else {
        "Bearer"
    };
    format!("{prefix} {key}")
}

/// The native Codex backend requires a streamed, unstored request with top-level instructions.
pub fn chatgpt_body(body: &[u8]) -> Result<Vec<u8>, String> {
    let mut body: Value = serde_json::from_slice(body).map_err(|_| "invalid account inference request")?;
    let object = body.as_object_mut().ok_or("account inference request must be an object")?;
    let mut instructions = object.get("instructions").and_then(Value::as_str).unwrap_or_default().to_owned();
    if let Some(input) = object.get_mut("input").and_then(Value::as_array_mut) {
        let mut retained = Vec::with_capacity(input.len());
        for message in input.drain(..) {
            if matches!(message.get("role").and_then(Value::as_str), Some("system" | "developer")) {
                let text = instruction_text(message.get("content"))?;
                if !text.is_empty() {
                    if !instructions.is_empty() {
                        instructions.push_str("\n\n");
                    }
                    instructions.push_str(&text);
                }
            } else {
                retained.push(message);
            }
        }
        *input = retained;
    }
    object.insert("instructions".into(), Value::String(instructions));
    object.insert("store".into(), Value::Bool(false));
    object.insert("stream".into(), Value::Bool(true));
    for key in ["max_output_tokens", "temperature", "top_p"] {
        object.remove(key);
    }
    serde_json::to_vec(&body).map_err(|_| "invalid account inference request".into())
}

fn instruction_text(content: Option<&Value>) -> Result<String, String> {
    match content {
        Some(Value::String(text)) => Ok(text.to_owned()),
        Some(Value::Array(parts)) => parts
            .iter()
            .map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| "account instructions must contain text only".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|parts| parts.join("\n")),
        None | Some(Value::Null) => Ok(String::new()),
        _ => Err("account instructions must contain text only".into()),
    }
}
