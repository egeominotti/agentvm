use super::Session;
use crate::domain::accounts::AccountKind;
use base64::Engine;
use serde_json::Value;

pub fn validate_token(token: &str) -> Result<(), String> {
    if token.is_empty() || token.len() > 32768 || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("account token missing or malformed; sign in again".into());
    }
    Ok(())
}
fn claims(token: &str) -> Value {
    token
        .split('.')
        .nth(1)
        .and_then(|s| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null)
}
pub fn parse_session(account: AccountKind, value: &Value) -> Result<Session, String> {
    if account == AccountKind::Chatgpt && value.get("auth_mode").and_then(Value::as_str).is_some_and(|m| m != "chatgpt")
    {
        return Err("local Codex is not signed in with a ChatGPT account".into());
    }
    let v = match account {
        AccountKind::Chatgpt => value.get("tokens").or_else(|| value.get("openai")).unwrap_or(value),
        AccountKind::Kilo => value.get("kilo").unwrap_or(value),
        AccountKind::Google => value.get("token").unwrap_or(value),
        _ => value,
    };
    let get = |names: &[&str]| names.iter().find_map(|k| v.get(k).and_then(Value::as_str)).map(str::to_owned);
    let access_token = get(&["access_token", "accessToken", "access", "token"])
        .or_else(|| (account == AccountKind::Kilo).then(|| get(&["key"])).flatten())
        .ok_or("local CLI account login missing")?;
    validate_token(&access_token)?;
    if account == AccountKind::Copilot && !["gho_", "ghu_", "github_pat_"].iter().any(|p| access_token.starts_with(p)) {
        return Err("unsupported GitHub token; use Copilot sign-in or a fine-grained Copilot Requests token".into());
    }
    let refresh_token = get(&["refresh_token", "refreshToken", "refresh"])
        .filter(|token| account != AccountKind::Kilo || !token.is_empty());
    if let Some(token) = &refresh_token {
        validate_token(token)?;
    }
    let jwt = claims(&access_token);
    if account == AccountKind::Chatgpt {
        let direct = [v.get("scope"), jwt.get("scope"), jwt.get("scp")].into_iter().flatten().any(|scope| {
            scope.as_str().is_some_and(|s| s.split_whitespace().any(|s| s == "chatgpt.tokens.use.direct"))
                || scope
                    .as_array()
                    .is_some_and(|items| items.iter().any(|v| v.as_str() == Some("chatgpt.tokens.use.direct")))
        });
        if direct {
            return Err("this ChatGPT token uses the direct API flow; connect a Codex CLI account instead".into());
        }
    }
    let id = get(&["id_token"]).map(|s| claims(&s)).unwrap_or(Value::Null);
    let account_id = get(&["account_id", "accountId"]).or_else(|| {
        id.pointer("/https:~1~1api.openai.com~1auth/chatgpt_account_id")
            .or_else(|| jwt.pointer("/https:~1~1api.openai.com~1auth/chatgpt_account_id"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    if account == AccountKind::Chatgpt && account_id.is_none() {
        return Err("ChatGPT account identity missing; sign in to Codex again".into());
    }
    if let Some(id) = &account_id {
        validate_token(id)?;
    }
    let milliseconds = ["expiry_date", "expiresAt", "expires"].iter().find_map(|k| v.get(k).and_then(Value::as_u64));
    let expires_at = milliseconds
        .map(|n| n / 1000)
        .or_else(|| v.get("expires_at").and_then(Value::as_u64))
        .or_else(|| v.get("expires_in").and_then(Value::as_u64).map(|n| super::now().saturating_add(n)))
        .or_else(|| jwt["exp"].as_u64())
        .unwrap_or(0);
    Ok(Session { access_token, refresh_token, account_id, expires_at })
}
fn read_json(path: &std::path::Path) -> Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|_| "local CLI login unavailable; sign in on the host and import again")?;
    if bytes.len() > 262144 {
        return Err("local CLI login file is too large".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "local CLI login file is malformed")?;
    let filtered = text.lines().filter(|line| !line.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    serde_json::from_str(&filtered).map_err(|_| "local CLI login file is malformed".into())
}
fn copilot_identity(value: &Value) -> Result<(&str, &str), String> {
    let active = value
        .get("lastLoggedInUser")
        .or_else(|| value.get("last_logged_in_user"))
        .or_else(|| {
            value
                .get("loggedInUsers")
                .or_else(|| value.get("logged_in_users"))
                .and_then(|v| v.as_array())
                .and_then(|v| v.first())
        })
        .ok_or("Copilot selected user missing; sign in on the host first")?;
    let host = active["host"].as_str().ok_or("Copilot selected host missing")?;
    let login = active["login"].as_str().filter(|s| !s.is_empty()).ok_or("Copilot selected user missing")?;
    if host != "https://github.com" {
        return Err("this GitHub host requires native Copilot sign-in support".into());
    }
    Ok((host, login))
}
pub fn copilot_references(path: &std::path::Path) -> Result<Vec<super::Reference>, String> {
    let value = read_json(path)?;
    let (host, login) = copilot_identity(&value)?;
    Ok(vec![
        super::Reference::File(path.into()),
        super::Reference::Keychain { service: "copilot-cli".into(), account: Some(format!("{host}:{login}")) },
        super::Reference::Keychain { service: "copilot-cli".into(), account: Some(format!("{host}:{login}:github")) },
    ])
}
pub fn read_reference(account: AccountKind, path: &std::path::Path) -> Result<Session, String> {
    let value = read_json(path)?;
    if account == AccountKind::Copilot && (value.get("authTokens").is_some() || value.get("copilot_tokens").is_some()) {
        let (host, login) = copilot_identity(&value)?;
        let token = value["authTokens"]
            .get(format!("{host}:{login}:github"))
            .or_else(|| value["copilot_tokens"].get(format!("{host}:{login}")))
            .ok_or("selected Copilot login missing")?;
        let token = if let Some(token) = token.as_str() { serde_json::json!({"token":token}) } else { token.clone() };
        return parse_session(account, &token);
    }
    parse_session(account, &value)
}
pub fn validate_copilot_endpoint(endpoint: &str) -> Result<(), String> {
    let url = url::Url::parse(endpoint).map_err(|_| "invalid Copilot account endpoint")?;
    let host = url.host_str().unwrap_or("");
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port().is_some()
        || !matches!(url.path(), "" | "/")
        || !(host == "api.githubcopilot.com" || host.ends_with(".githubcopilot.com"))
    {
        return Err("untrusted Copilot account endpoint".into());
    }
    Ok(())
}
