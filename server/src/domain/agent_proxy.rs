//! Fixed-upstream inference routing and streaming credential redaction.

use super::providers::ApiProtocol;
use url::Url;

/// CLI aliases only apply to the original provider API, including when a saved endpoint changes.
pub fn native_model_defaults(kind: super::agents::AgentKind, config: &super::providers::ProviderConfig) -> bool {
    use super::agents::AgentKind;
    use super::providers::Provider;
    let provider = match kind {
        AgentKind::Claude => Provider::Anthropic,
        AgentKind::Codex => Provider::Openai,
        AgentKind::Gemini => Provider::Google,
        _ => return false,
    };
    let default = provider.config();
    config.protocol == default.protocol
        && endpoint_url(config.endpoint.trim_end_matches('/')).ok()
            == endpoint_url(default.endpoint.trim_end_matches('/')).ok()
}

fn endpoint_url(endpoint: &str) -> Result<Url, String> {
    let url = Url::parse(endpoint).map_err(|_| "invalid provider endpoint")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("invalid provider endpoint".into());
    }
    Ok(url)
}

pub fn guest_endpoint(endpoint: &str) -> Result<String, String> {
    let url = endpoint_url(endpoint)?;
    Ok(format!("http://127.0.0.1:5001{}", url.path().trim_end_matches('/')))
}

pub fn target_url(endpoint: &str, path_query: &str) -> Result<Url, String> {
    let mut base = endpoint_url(endpoint)?;
    if !path_query.starts_with('/') || path_query.starts_with("//") || path_query.contains(['\\', '#', '\r', '\n']) {
        return Err("relay accepts relative API paths only".into());
    }
    let (path, query) = path_query.split_once('?').unwrap_or((path_query, ""));
    let lower = path.to_ascii_lowercase();
    if lower.contains("%2e")
        || lower.contains("%2f")
        || lower.contains("%5c")
        || path.split('/').any(|p| p == "." || p == "..")
    {
        return Err("relay path traversal refused".into());
    }
    let prefix = base.path().trim_end_matches('/');
    if !prefix.is_empty() && path != prefix && !path.starts_with(&format!("{prefix}/")) {
        return Err("relay path is outside the provider API".into());
    }
    base.set_path(path);
    let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes())
        .filter(|(key, _)| {
            !matches!(key.to_ascii_lowercase().as_str(), "key" | "api_key" | "apikey" | "access_token" | "token")
        })
        .collect();
    base.set_query(None);
    if !pairs.is_empty() {
        base.query_pairs_mut().extend_pairs(pairs);
    }
    Ok(base)
}

pub fn authentication(protocol: ApiProtocol, key: &str, oauth: bool) -> Vec<(String, String)> {
    if key.is_empty() {
        return Vec::new();
    }
    let (name, value) = match protocol {
        ApiProtocol::Openai => ("authorization", format!("Bearer {key}")),
        ApiProtocol::Anthropic if oauth => ("authorization", format!("Bearer {key}")),
        ApiProtocol::Anthropic => ("x-api-key", key.to_owned()),
        ApiProtocol::Google => ("x-goog-api-key", key.to_owned()),
    };
    vec![(name.into(), value)]
}

pub fn request_header(name: &str) -> bool {
    matches!(
        name,
        "content-type"
            | "accept"
            | "user-agent"
            | "anthropic-version"
            | "anthropic-beta"
            | "openai-beta"
            | "x-goog-api-client"
    ) || name.starts_with("x-stainless-")
}

pub fn response_header(name: &str) -> bool {
    matches!(name, "content-type" | "cache-control" | "retry-after" | "x-request-id" | "request-id")
        || name.starts_with("anthropic-ratelimit-")
        || name.starts_with("x-ratelimit-")
}

/// Only a suffix that could complete the credential is held back; normal SSE flows immediately.
pub struct StreamRedactor {
    key: Vec<u8>,
    pending: Vec<u8>,
}

impl StreamRedactor {
    pub fn new(key: Vec<u8>) -> Self {
        Self { key, pending: Vec::new() }
    }
    pub fn push(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.pending.extend_from_slice(bytes);
        if self.key.is_empty() {
            return std::mem::take(&mut self.pending);
        }
        let mut from = 0;
        while let Some(offset) = self.pending[from..].windows(self.key.len()).position(|v| v == self.key) {
            let at = from + offset;
            self.pending.splice(at..at + self.key.len(), b"[REDACTED]".iter().copied());
            from = at + b"[REDACTED]".len();
        }
        let keep = (1..self.key.len().min(self.pending.len() + 1))
            .rev()
            .find(|&n| self.pending.ends_with(&self.key[..n]))
            .unwrap_or(0);
        self.pending.drain(..self.pending.len() - keep).collect()
    }
    pub fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending)
    }
}
