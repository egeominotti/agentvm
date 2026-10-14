//! Host account sessions. Network errors never include token responses or credential URLs.
pub mod login;
mod parse;
use crate::domain::accounts::AccountKind;
pub use parse::{copilot_references, parse_session, read_reference, validate_copilot_endpoint};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub account_id: Option<String>,
    pub expires_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub enum Reference {
    File(std::path::PathBuf),
    Keychain { service: String, account: Option<String> },
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Connection {
    pub session: Option<Session>,
    pub reference: Option<Reference>,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}
pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| "cannot initialize account HTTP client".into())
}
pub async fn refresh(account: AccountKind, session: &Session) -> Result<Session, String> {
    let token = session.refresh_token.as_ref().ok_or("account session expired; sign in again")?;
    let response = match account {
        AccountKind::Chatgpt => client()?.post("https://auth.openai.com/oauth/token")
            .json(&serde_json::json!({"grant_type":"refresh_token","client_id":login::CODEX_CLIENT,"refresh_token":token})).send().await,
        AccountKind::Google => {
            let config = login::google_client::load().await?;
            client()?.post("https://oauth2.googleapis.com/token")
                .form(&[("grant_type","refresh_token"),("client_id",config.client_id.as_str()),("client_secret",config.client_secret.as_str()),("refresh_token",token.as_str())]).send().await
        },
        _ => return Err("account session expired; sign in again".into()),
    }.map_err(|_| "account refresh could not reach the authentication service")?;
    let value = checked_json(response).await?;
    let mut next = parse_session(account, &value)?;
    next.refresh_token = next.refresh_token.or_else(|| session.refresh_token.clone());
    next.account_id = next.account_id.or_else(|| session.account_id.clone());
    if next.account_id != session.account_id {
        return Err("account changed during refresh; sign in again".into());
    }
    Ok(next)
}
pub async fn checked_json(response: reqwest::Response) -> Result<serde_json::Value, String> {
    if !response.status().is_success() {
        return Err(format!(
            "account authentication rejected (HTTP {}); sign in again or check account policy",
            response.status().as_u16()
        ));
    }
    response.json().await.map_err(|_| "authentication service returned invalid account data".into())
}
pub async fn copilot_token(token: &str) -> Result<(Session, String), String> {
    let response = client()?
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Authorization", format!("token {token}"))
        .header("User-Agent", "agentvm/0.2.0")
        .header("X-GitHub-Api-Version", "2025-04-01")
        .send()
        .await
        .map_err(|_| "could not reach GitHub Copilot authentication")?;
    let value = checked_json(response).await?;
    let endpoint = value.pointer("/endpoints/api").and_then(|v| v.as_str()).unwrap_or("https://api.githubcopilot.com");
    validate_copilot_endpoint(endpoint)?;
    let access = value["token"].as_str().ok_or("Copilot token missing")?.to_owned();
    parse::validate_token(&access)?;
    let expires_at = value["expires_at"].as_u64().ok_or("Copilot token expiry missing")?;
    let refresh_at = now().saturating_add(value["refresh_in"].as_u64().unwrap_or(300));
    Ok((
        Session { access_token: access, refresh_token: None, account_id: None, expires_at: expires_at.min(refresh_at) },
        endpoint.into(),
    ))
}
