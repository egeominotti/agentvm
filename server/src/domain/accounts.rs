//! Account choices and non-secret connection descriptions.
use serde::{Deserialize, Serialize};

/// Public installed-app metadata retained on the host; never serialized or logged.
pub struct GoogleOAuthClient {
    pub client_id: String,
    pub client_secret: String,
}
impl GoogleOAuthClient {
    pub fn validate(&self) -> Result<(), String> {
        let id = self.client_id.strip_suffix(".apps.googleusercontent.com");
        let valid_id = id.is_some_and(|id| {
            let Some((number, suffix)) = id.split_once('-') else { return false };
            !number.is_empty()
                && number.bytes().all(|b| b.is_ascii_digit())
                && !suffix.is_empty()
                && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
        });
        let valid_secret = self.client_secret.strip_prefix("GOCSPX-").is_some_and(|secret| {
            (16..=128).contains(&secret.len())
                && secret.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        });
        if !valid_id || self.client_id.len() > 256 || !valid_secret {
            return Err("Google public OAuth metadata is invalid; retry later".into());
        }
        Ok(())
    }
}
pub fn parse_google_oauth_source(source: &str) -> Result<GoogleOAuthClient, String> {
    if source.len() > 131072 {
        return Err("Google public OAuth metadata is too large".into());
    }
    let extract = |name: &str| -> Result<String, String> {
        let prefix = format!("const {name} =");
        let mut found = None;
        let mut offset = 0;
        let mut block_comment = false;
        for line in source.split_inclusive('\n') {
            if block_comment || line.trim_start().starts_with("/*") {
                block_comment = !line.contains("*/");
                offset += line.len();
                continue;
            }
            if let Some(rest) = line.trim_start().strip_prefix(&prefix) {
                if found.is_some() {
                    return Err("Google public OAuth metadata is ambiguous".into());
                }
                let start = offset + line.len() - rest.len();
                let value = source[start..].trim_start();
                let quote = value
                    .chars()
                    .next()
                    .filter(|c| matches!(c, '\'' | '"'))
                    .ok_or("Google public OAuth metadata is malformed")?;
                let end = value[1..].find(quote).ok_or("Google public OAuth metadata is malformed")? + 1;
                let token = &value[1..end];
                if token.is_empty()
                    || token.len() > 256
                    || !token.bytes().all(|b| b.is_ascii_graphic() && b != b'\\')
                    || !value[end + 1..].trim_start().starts_with(';')
                {
                    return Err("Google public OAuth metadata is malformed".into());
                }
                found = Some(token.to_owned());
            }
            offset += line.len();
        }
        found.ok_or_else(|| "Google public OAuth metadata is unavailable; retry later".into())
    };
    Ok(GoogleOAuthClient { client_id: extract("OAUTH_CLIENT_ID")?, client_secret: extract("OAUTH_CLIENT_SECRET")? })
}

/// Host-only authentication supplied to a relay; deliberately not serializable.
pub struct AccountAccess {
    pub account: AccountKind,
    pub config: super::providers::ProviderConfig,
    pub credential: crate::secret::Secret,
    pub catalog_credential: Option<crate::secret::Secret>,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum AccountKind {
    Claude,
    Chatgpt,
    Google,
    Copilot,
    Kilo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum AgentAuth {
    Account,
    Api,
}

impl AccountKind {
    /// Copilot's model catalog authenticates the original GitHub login, before inference-token exchange.
    pub fn catalog_credential(self, token: &str) -> Option<crate::secret::Secret> {
        (self == Self::Copilot).then(|| crate::secret::Secret::new(token.to_owned()))
    }
    pub const ALL: [Self; 5] = [Self::Claude, Self::Chatgpt, Self::Google, Self::Copilot, Self::Kilo];
    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Chatgpt => "chatgpt",
            Self::Google => "google",
            Self::Copilot => "copilot",
            Self::Kilo => "kilo",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude account",
            Self::Chatgpt => "ChatGPT account",
            Self::Google => "Google Code Assist account",
            Self::Copilot => "GitHub Copilot account",
            Self::Kilo => "Kilo account and credits",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AccountStatus {
    pub id: AccountKind,
    pub name: String,
    pub connected: bool,
    pub source: Option<String>,
    pub error: Option<String>,
    pub can_import: bool,
    pub can_login: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct LoginStart {
    pub id: String,
    pub verification_url: String,
    pub user_code: Option<String>,
    pub expires_in: u64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum LoginState {
    Pending,
    Connected,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct LoginStatus {
    pub id: String,
    pub status: LoginState,
    pub error: Option<String>,
}

/// Match Kilo CLI's login-time cloud selection without changing an existing imported organization.
pub fn kilo_organization(profile: &serde_json::Value) -> Result<Option<String>, String> {
    let organizations = profile.get("organizations").and_then(serde_json::Value::as_array);
    let selected = profile.get("selectedOrganizationId").and_then(serde_json::Value::as_str);
    let eligible = |id: &str| {
        organizations
            .is_some_and(|orgs| orgs.iter().any(|org| org.get("id").and_then(serde_json::Value::as_str) == Some(id)))
    };
    let required = profile.get("hasPersonalAccount").and_then(serde_json::Value::as_bool) == Some(false);
    let id = selected
        .filter(|id| eligible(id))
        .or_else(|| required.then(|| organizations?.first()?.get("id")?.as_str()).flatten());
    if let Some(id) = id {
        if id.is_empty() || id.len() > 32768 || !id.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("Kilo organization is invalid; sign in again".into());
        }
        return Ok(Some(id.into()));
    }
    if required {
        return Err("Kilo account has no available organization; check organization access and sign in again".into());
    }
    Ok(None)
}

/// Unrelated and mismatched requests never consume the one-use authorization code.
pub fn callback_code(path: &str, expected_state: &str) -> Result<Option<String>, String> {
    let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{path}")) else { return Ok(None) };
    if url.path() != "/oauth2callback" {
        return Ok(None);
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let states: Vec<_> = pairs.iter().filter(|(k, _)| k == "state").collect();
    if states.len() != 1 || states[0].1 != expected_state {
        return Ok(None);
    }
    if pairs.iter().any(|(k, _)| k == "error") {
        return Err("Google sign-in denied; try again".into());
    }
    let codes: Vec<_> = pairs.iter().filter(|(k, _)| k == "code").collect();
    if codes.len() != 1 || codes[0].1.is_empty() {
        return Err("Google authorization code missing".into());
    }
    Ok(Some(codes[0].1.to_string()))
}
