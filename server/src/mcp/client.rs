//! HTTP client for the already-running loopback daemon; no engine startup side effects.
use futures::StreamExt;
use reqwest::{Client, Method, Response};
use serde_json::{Value, json};
use url::{Host, Url};

pub const RESULT_LIMIT: usize = 4 << 20;

pub fn validate_url(input: &str) -> Result<Url, String> {
    let url = Url::parse(input).map_err(|_| "invalid AgentVM URL")?;
    let loopback = match url.host() {
        Some(Host::Domain(host)) => host == "localhost",
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    if !loopback
        || url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("AgentVM URL must be an HTTP loopback origin without credentials, path or query".into());
    }
    Ok(url)
}

pub fn validate_path(method: &str, path: &str) -> Result<Method, String> {
    let method = match method {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        _ => return Err("method must be GET, POST, PUT or DELETE".into()),
    };
    if !path.starts_with("/api/") || path.contains(['#', '\\', '\r', '\n']) || path.len() > 8192 {
        return Err("path must be a relative /api/ route without a fragment".into());
    }
    let raw = path.split('?').next().unwrap();
    let mut decoded = raw.to_owned();
    for _ in 0..4 {
        let mut bytes = Vec::new();
        let mut iter = decoded.bytes();
        while let Some(byte) = iter.next() {
            if byte == b'%' {
                let hi = iter.next().and_then(|b| (b as char).to_digit(16)).ok_or("invalid path encoding")?;
                let lo = iter.next().and_then(|b| (b as char).to_digit(16)).ok_or("invalid path encoding")?;
                bytes.push((hi * 16 + lo) as u8);
            } else {
                bytes.push(byte);
            }
        }
        let next = String::from_utf8(bytes).map_err(|_| "invalid path encoding")?;
        if next == decoded {
            break;
        }
        decoded = next;
    }
    if decoded.contains(['%', '\\', '?', '#'])
        || decoded.chars().any(char::is_control)
        || decoded.split('/').skip(1).any(|s| matches!(s, "" | "." | ".."))
    {
        return Err("path contains unsafe segments or encoding".into());
    }
    if decoded == "/api/changes"
        || decoded == "/api/snapshots/import"
        || ["/events", "/pty", "/export", "/upload"].iter().any(|suffix| decoded.ends_with(suffix))
    {
        return Err("this route streams data; use structured terminal reads or agentvm_transfer".into());
    }
    Ok(method)
}

#[derive(Clone)]
pub struct ApiClient {
    pub(crate) origin: Url,
    pub(crate) http: Client,
}

impl ApiClient {
    pub fn new(origin: &str) -> Result<Self, String> {
        Ok(Self {
            origin: validate_url(origin)?,
            http: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .map_err(|_| "could not construct AgentVM client")?,
        })
    }

    pub async fn api(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
        let method = validate_path(method, path)?;
        let url = self.origin.join(path).map_err(|_| "invalid API path")?;
        let mut request = self.http.request(method, url);
        if let Some(body) = body {
            request = request.header("content-type", "application/json").body(body.to_string());
        }
        let response = request.send().await.map_err(|_| "cannot reach AgentVM; start the app or daemon first")?;
        result(response).await
    }
}

pub(crate) async fn result(response: Response) -> Result<Value, String> {
    let status = response.status();
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "failed reading AgentVM response")?;
        if bytes.len() + chunk.len() > RESULT_LIMIT {
            return Err("API result exceeds 4 MiB; use a chunked or artifact transfer tool".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let data = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    if !status.is_success() {
        return Err(format!("AgentVM HTTP {}: {}", status.as_u16(), data));
    }
    Ok(json!({"status":status.as_u16(),"data":data}))
}
