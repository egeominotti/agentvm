use super::{Auth, Upstream, response};
use crate::domain::account_proxy::{
    account_authorization, account_request_header, account_target, chatgpt_body, uses_catalog_credential,
};
use crate::domain::accounts::AccountKind;
use crate::domain::agent_proxy::{authentication, request_header, target_url};
use crate::domain::agents::AgentKind;
use crate::domain::provider_models::{is_opencode, preset_catalog};
use crate::domain::providers::ProviderConfig;
use crate::secret::Secret;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::Response;
use std::sync::Arc;

const REQUEST_MAX: usize = 64 << 20;
struct Credentials {
    config: ProviderConfig,
    key: Secret,
    headers: Vec<(String, String)>,
    account: Option<AccountKind>,
    oauth: bool,
}

pub(super) async fn forward(State(state): State<Arc<Upstream>>, req: Request) -> Response {
    let path = req.uri().path_and_query().map_or("/", |p| p.as_str());
    let checked = match &state.auth {
        Auth::Api { .. } => target_url(&state.config.endpoint, path),
        Auth::Account { account, .. } => account_target(*account, &state.config, path),
    };
    if let Err(error) = checked {
        return response::error(StatusCode::BAD_REQUEST, &error);
    }
    let path = path.to_owned();
    let (parts, body) = req.into_parts();
    let catalog = matches!(&state.auth, Auth::Account { account, .. }
        if uses_catalog_credential(*account, parts.method.as_str(), &path));
    let mut body = match axum::body::to_bytes(body, REQUEST_MAX).await {
        Ok(bytes) => bytes.to_vec(),
        Err(_) => return response::error(StatusCode::PAYLOAD_TOO_LARGE, "inference request exceeds 64 MiB"),
    };
    if matches!(&state.auth, Auth::Account { account: AccountKind::Chatgpt, .. })
        && parts.method == axum::http::Method::POST
        && parts.uri.path().ends_with("/responses")
    {
        body = match chatgpt_body(&body) {
            Ok(body) => body,
            Err(error) => return response::error(StatusCode::BAD_REQUEST, &error),
        };
    }
    for attempt in 0..2 {
        let credentials = match credentials(&state, attempt != 0, catalog).await {
            Ok(value) => value,
            Err(error) => return response::error(StatusCode::SERVICE_UNAVAILABLE, &error),
        };
        let url = match credentials.account {
            Some(account) => account_target(account, &credentials.config, &path),
            None => target_url(&credentials.config.endpoint, &path),
        };
        let url = match url {
            Ok(url) => url,
            Err(error) => return response::error(StatusCode::BAD_GATEWAY, &error),
        };
        if credentials.config.requires_key && credentials.key.expose().is_empty() {
            return response::error(
                StatusCode::SERVICE_UNAVAILABLE,
                "provider credential unavailable: connect it in AgentVM Settings",
            );
        }
        let models = parts.method == axum::http::Method::GET && url.path().ends_with("/models");
        if models
            && credentials.account.is_none()
            && let Some(catalog) = preset_catalog(&credentials.config)
        {
            return response::catalog_json(catalog, &credentials.key);
        }
        let mut request = state.client.request(parts.method.clone(), url).body(body.clone());
        for (name, value) in &parts.headers {
            let allowed = if credentials.account.is_some() {
                account_request_header(name.as_str())
            } else {
                request_header(name.as_str())
            };
            if allowed {
                request = request.header(name, value);
            }
        }
        let auth = match credentials.account {
            Some(account) if account != AccountKind::Claude => vec![(
                "authorization".into(),
                account_authorization(account, &credentials.config, credentials.key.expose(), catalog),
            )],
            _ => authentication(credentials.config.protocol, credentials.key.expose(), credentials.oauth),
        };
        for (name, value) in auth.into_iter().chain(credentials.headers) {
            request = request.header(name, value);
        }
        if credentials.account.is_none() && is_opencode(&credentials.config) {
            if !parts.headers.contains_key("user-agent") {
                request = request.header("user-agent", "AgentVM");
            }
            if !parts.headers.contains_key("x-opencode-session") {
                if let Some((_, session)) = &state.context {
                    request = request.header("x-opencode-session", session);
                }
            } else if let Some(session) = parts.headers.get("x-opencode-session") {
                request = request.header("x-opencode-session", session);
            }
        }
        if credentials.oauth {
            let beta = parts.headers.get("anthropic-beta").and_then(|v| v.to_str().ok()).unwrap_or("");
            if !beta.split(',').any(|v| v.trim() == "oauth-2025-04-20") {
                request = request.header(
                    "anthropic-beta",
                    if beta.is_empty() { "oauth-2025-04-20".into() } else { format!("{beta},oauth-2025-04-20") },
                );
            }
        }
        let upstream = match request.send().await {
            Ok(response) => response,
            Err(_) => {
                return response::error(
                    StatusCode::BAD_GATEWAY,
                    "the configured inference service could not be reached",
                );
            }
        };
        if upstream.status().is_redirection() {
            return response::error(
                StatusCode::BAD_GATEWAY,
                "inference service redirected: configure its final API endpoint",
            );
        }
        if attempt == 0 && credentials.account.is_some() && upstream.status() == StatusCode::UNAUTHORIZED {
            continue;
        }
        if models && credentials.account.is_none() && is_opencode(&credentials.config) && upstream.status().is_success()
        {
            let kind = state.context.as_ref().map_or(AgentKind::Custom, |(kind, _)| *kind);
            return response::model_catalog(upstream, credentials.key, credentials.config, kind).await;
        }
        return response::stream(upstream, credentials.key);
    }
    response::error(StatusCode::UNAUTHORIZED, "account access expired: sign in again on the Mac")
}

async fn credentials(state: &Upstream, force: bool, catalog: bool) -> Result<Credentials, String> {
    match &state.auth {
        Auth::Api { key, oauth } => Ok(Credentials {
            config: state.config.clone(),
            key: Secret::new(key.expose().to_owned()),
            headers: Vec::new(),
            account: None,
            oauth: *oauth,
        }),
        Auth::Account { account, refresh } => {
            let access = refresh(force).await?;
            if access.account != *account {
                return Err("account connection does not match this VM".into());
            }
            let key = if catalog {
                access.catalog_credential.ok_or("Copilot catalog credential missing; reconnect the host account")?
            } else {
                access.credential
            };
            let oauth = *account == AccountKind::Claude && key.expose().starts_with("sk-ant-oat");
            Ok(Credentials { config: access.config, key, headers: access.headers, account: Some(*account), oauth })
        }
    }
}
