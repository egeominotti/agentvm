//! Official CLI device flows. Device IDs and issued tokens stay on the host.
use super::{Session, checked_json, client, parse_session};
use crate::domain::accounts::{AccountKind, LoginStart};
use serde_json::{Value, json};
pub mod google;
pub mod google_client;
pub const CODEX_CLIENT: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const COPILOT_CLIENT: &str = "Ov23ctDVkRmgkPke0Mmm";

pub enum Flow {
    Device { account: AccountKind, device: String, code: String, interval: u64, expires: u64 },
    Google(google::Flow),
}
pub fn nonce() -> Result<String, String> {
    use base64::Engine;
    use std::io::Read;
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| "secure randomness unavailable")?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}
fn text(v: &Value, k: &str) -> Result<String, String> {
    v[k].as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("authentication service omitted {k}"))
}
pub async fn start(account: AccountKind) -> Result<(LoginStart, Flow), String> {
    if account == AccountKind::Google {
        return google::start().await;
    }
    let c = client()?;
    let response = match account {
        AccountKind::Chatgpt => {
            c.post("https://auth.openai.com/api/accounts/deviceauth/usercode")
                .json(&json!({"client_id":CODEX_CLIENT}))
                .send()
                .await
        }
        AccountKind::Copilot => {
            c.post("https://github.com/login/device/code")
                .header("Accept", "application/json")
                .form(&[("client_id", COPILOT_CLIENT), ("scope", "read:user")])
                .send()
                .await
        }
        AccountKind::Kilo => c.post("https://api.kilo.ai/api/device-auth/codes").send().await,
        AccountKind::Claude => {
            return Err("Connect a Claude account using claude setup-token and the Claude token setting".into());
        }
        AccountKind::Google => unreachable!(),
    }
    .map_err(|_| "could not start host account sign-in")?;
    let v = checked_json(response).await?;
    let (device, code, url, expires) = match account {
        AccountKind::Chatgpt => (
            text(&v, "device_auth_id")?,
            text(&v, "user_code").or_else(|_| text(&v, "usercode"))?,
            "https://auth.openai.com/codex/device".into(),
            900,
        ),
        AccountKind::Copilot => (
            text(&v, "device_code")?,
            text(&v, "user_code")?,
            text(&v, "verification_uri")?,
            v["expires_in"].as_u64().unwrap_or(900),
        ),
        AccountKind::Kilo => {
            let code = text(&v, "code")?;
            (code.clone(), code, text(&v, "verificationUrl")?, v["expiresIn"].as_u64().unwrap_or(900))
        }
        _ => unreachable!(),
    };
    let interval = v["interval"]
        .as_u64()
        .or_else(|| v["interval"].as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(5)
        .clamp(1, 60);
    let expires = expires.clamp(1, 900);
    let start = LoginStart { id: nonce()?, verification_url: url, user_code: Some(code.clone()), expires_in: expires };
    Ok((start, Flow::Device { account, device, code, interval, expires: super::now() + expires }))
}
pub async fn complete(flow: Flow) -> Result<Session, String> {
    let Flow::Device { account, device, code, mut interval, expires } = flow else {
        if let Flow::Google(flow) = flow {
            return google::complete(flow).await;
        }
        unreachable!()
    };
    loop {
        if super::now() >= expires {
            return Err("account sign-in expired; start sign-in again".into());
        }
        let c = client()?;
        let response = match account {
            AccountKind::Chatgpt => {
                c.post("https://auth.openai.com/api/accounts/deviceauth/token")
                    .json(&json!({"device_auth_id":device,"user_code":code}))
                    .send()
                    .await
            }
            AccountKind::Copilot => {
                c.post("https://github.com/login/oauth/access_token")
                    .header("Accept", "application/json")
                    .form(&[
                        ("client_id", COPILOT_CLIENT),
                        ("device_code", device.as_str()),
                        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ])
                    .send()
                    .await
            }
            AccountKind::Kilo => {
                c.get(format!(
                    "https://api.kilo.ai/api/device-auth/codes/{}",
                    url::form_urlencoded::byte_serialize(device.as_bytes()).collect::<String>()
                ))
                .send()
                .await
            }
            _ => unreachable!(),
        }
        .map_err(|_| "could not reach host account sign-in")?;
        if account == AccountKind::Chatgpt && matches!(response.status().as_u16(), 403 | 404) {
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
            continue;
        }
        let mut v = checked_json(response).await?;
        if account == AccountKind::Chatgpt {
            let response = c
                .post("https://auth.openai.com/oauth/token")
                .form(&[
                    ("grant_type", "authorization_code"),
                    ("client_id", CODEX_CLIENT),
                    ("code", text(&v, "authorization_code")?.as_str()),
                    ("code_verifier", text(&v, "code_verifier")?.as_str()),
                    ("redirect_uri", "https://auth.openai.com/deviceauth/callback"),
                ])
                .send()
                .await
                .map_err(|_| "could not exchange ChatGPT sign-in")?;
            v = checked_json(response).await?;
            return parse_session(account, &v);
        }
        if account == AccountKind::Copilot {
            if v.get("access_token").is_some() {
                return parse_session(account, &v);
            }
            match v["error"].as_str() {
                Some("authorization_pending") => {}
                Some("slow_down") => interval = (interval + 5).min(60),
                _ => return Err("GitHub sign-in denied or expired; try again".into()),
            }
        } else {
            match v["status"].as_str() {
                Some("approved") => {
                    let mut session = parse_session(account, &v)?;
                    let profile = c
                        .get("https://api.kilo.ai/api/profile")
                        .bearer_auth(&session.access_token)
                        .header("Content-Type", "application/json")
                        .send()
                        .await
                        .map_err(|_| "could not load Kilo account profile; retry sign-in")?;
                    let profile = checked_json(profile).await?;
                    session.account_id = crate::domain::accounts::kilo_organization(&profile)?;
                    return Ok(session);
                }
                Some("denied" | "expired") => return Err("Kilo sign-in denied or expired; try again".into()),
                _ => {}
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
    }
}
