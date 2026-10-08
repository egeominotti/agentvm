//! Who may use the server: other users of this Mac reach 127.0.0.1 too. The browser presents the
//! API token as an HttpOnly cookie (set once by the link printed at start-up) or a Bearer header.

use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};

use crate::app::supervisor::AppCtx;

const COOKIE: &str = "agentvm_token";
const PROXY_COOKIE: &str = "agentvm_proxy";
const PROXY_PARAM: &str = "agentvm_token";

pub enum Access {
    Granted,
    /// Logged in through a link: set the cookie and go to `location` (the URL without the token).
    SetCookie {
        cookie: String,
        location: String,
    },
    Denied {
        page: bool,
    },
}

impl IntoResponse for Access {
    fn into_response(self) -> Response {
        match self {
            Access::Granted => StatusCode::OK.into_response(),
            Access::SetCookie { cookie, location } => {
                let mut res = StatusCode::SEE_OTHER.into_response();
                let h = res.headers_mut();
                h.insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).expect("token is hex"));
                h.insert(header::LOCATION, HeaderValue::from_str(&location).unwrap_or(HeaderValue::from_static("/")));
                h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
                res
            }
            Access::Denied { page: true } => (StatusCode::UNAUTHORIZED, Html(LOCKED)).into_response(),
            Access::Denied { page: false } => {
                (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({ "error": "open agentvm from the link printed by agentvm-server (agentvm-server --url)" })))
                    .into_response()
            }
        }
    }
}

/// The dashboard and its API. Static files (styles, scripts, fonts, logo) are public.
pub fn dashboard_access(ctx: &AppCtx, req: &Request) -> Access {
    let path = req.uri().path();
    if path == "/logo.svg" || path.starts_with("/assets/") || path.starts_with("/vendor/") {
        return Access::Granted;
    }
    let token = ctx.api_token.expose();
    if bearer(req).is_some_and(|t| same(t, token)) || cookie(req, COOKIE).is_some_and(|t| same(t, token)) {
        return Access::Granted;
    }
    if path == "/"
        && let Some(given) = query(req, "token")
    {
        return if same(&given, token) {
            Access::SetCookie { cookie: session_cookie(COOKIE, token), location: "/".into() }
        } else {
            Access::Denied { page: true }
        };
    }
    Access::Denied { page: path == "/" }
}

/// A VM's web service: the proxy token (from the dashboard's links) or the API token.
pub fn proxy_access(ctx: &AppCtx, req: &Request) -> Access {
    let (proxy, api) = (ctx.proxy_token.expose(), ctx.api_token.expose());
    let ok = |t: &str| same(t, proxy) || same(t, api);
    if cookie(req, PROXY_COOKIE).is_some_and(ok) || bearer(req).is_some_and(ok) {
        return Access::Granted;
    }
    if let Some(given) = query(req, PROXY_PARAM)
        && ok(&given)
    {
        return Access::SetCookie {
            cookie: session_cookie(PROXY_COOKIE, proxy),
            location: without_param(req, PROXY_PARAM),
        };
    }
    Access::Denied { page: true }
}

fn session_cookie(name: &str, token: &str) -> String {
    format!("{name}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=31536000")
}

fn bearer(req: &Request) -> Option<&str> {
    req.headers().get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ")
}

fn cookie<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|kv| kv.trim().strip_prefix(name)?.strip_prefix('='))
}

fn query(req: &Request, name: &str) -> Option<String> {
    req.uri().query()?.split('&').find_map(|kv| kv.strip_prefix(name)?.strip_prefix('=').map(str::to_owned))
}

fn without_param(req: &Request, name: &str) -> String {
    let rest: Vec<&str> = req
        .uri()
        .query()
        .unwrap_or_default()
        .split('&')
        .filter(|kv| !kv.is_empty() && !kv.starts_with(&format!("{name}=")))
        .collect();
    let path = req.uri().path();
    if rest.is_empty() { path.to_owned() } else { format!("{path}?{}", rest.join("&")) }
}

/// Constant time: how much of a token matched must not leak through timing.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

const LOCKED: &str = r#"<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<title>agentvm</title><link rel=icon href=/logo.svg>
<body style="margin:0;min-height:100vh;display:grid;place-items:center;background:#08090a;color:#eeeef0;font:14px/1.6 -apple-system,system-ui,sans-serif">
<main style="max-width:440px;padding:24px"><img src=/logo.svg width=32 height=32 alt="">
<h1 style="font-size:16px;font-weight:600;margin:14px 0 6px">Open agentvm from its link</h1>
<p style="color:#9b9ea6;margin:0 0 14px">For your safety the dashboard only opens with the private link agentvm prints when it starts. In a terminal:</p>
<pre style="background:#16171a;border:1px solid #2a2b30;border-radius:7px;padding:10px 12px;margin:0;color:#eeeef0">agentvm-server --url</pre>
<p style="color:#63666e;font-size:12.5px;margin:14px 0 0">Open that link once in this browser; it remembers it.</p></main>"#;
