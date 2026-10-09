//! Reverse proxy for `<port>.<vm>.localhost`: every VM can serve the same ports, each under its own
//! name. HTTP and WebSocket upgrades (dev servers, hot reload) pass through.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use hyper_util::rt::TokioIo;

use crate::app::proxy::{self, ProxyError};
use crate::app::supervisor::AppCtx;

pub async fn forward(ctx: Arc<AppCtx>, port: u16, vm: String, mut req: Request) -> Response {
    let stream = match proxy::connect(&ctx, &vm, port).await {
        Ok(s) => s,
        Err(e @ ProxyError::NoSuchMachine(_)) => return page(StatusCode::NOT_FOUND, &e.to_string()),
        Err(e) => return page(StatusCode::BAD_GATEWAY, &e.to_string()),
    };
    let Ok((mut sender, conn)) = hyper::client::conn::http1::handshake(TokioIo::new(stream)).await else {
        return page(StatusCode::BAD_GATEWAY, &format!("port {port} in {vm} does not speak HTTP"));
    };
    tokio::spawn(conn.with_upgrades());

    let public = req.headers().get(header::HOST).cloned();
    let inside = format!("localhost:{port}");
    // Dev servers (Vite, Next) accept only their own host and origin: the VM's own page gets it,
    // another site keeps its origin, so the service can refuse it.
    req.headers_mut().insert(header::HOST, HeaderValue::from_str(&inside).expect("valid header"));
    let public_host = public.as_ref().and_then(|h| h.to_str().ok()).unwrap_or_default().to_owned();
    if let Some(origin) = req.headers().get(header::ORIGIN).and_then(|o| o.to_str().ok()).map(str::to_owned) {
        let mapped = crate::domain::hostname::inside_origin(&origin, &public_host, port);
        if let Ok(v) = HeaderValue::from_str(&mapped) {
            req.headers_mut().insert(header::ORIGIN, v);
        }
    }
    *req.uri_mut() = req.uri().path_and_query().map_or("/", |p| p.as_str()).parse().expect("valid path");
    let client_upgrade = req.headers().contains_key(header::UPGRADE).then(|| hyper::upgrade::on(&mut req));

    let mut res = match sender.send_request(req).await {
        Ok(r) => r,
        Err(e) => return page(StatusCode::BAD_GATEWAY, &format!("port {port} in {vm}: {e}")),
    };
    if res.status() == StatusCode::SWITCHING_PROTOCOLS
        && let Some(client) = client_upgrade
    {
        let backend = hyper::upgrade::on(&mut res);
        tokio::spawn(async move {
            if let (Ok(c), Ok(b)) = (client.await, backend.await) {
                let _ = tokio::io::copy_bidirectional(&mut TokioIo::new(c), &mut TokioIo::new(b)).await;
            }
        });
    }
    // Redirects to the VM's own localhost come back to the same proxied name.
    if let (Some(loc), Some(public)) = (res.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()), public) {
        let public = public.to_str().unwrap_or_default();
        let fixed = loc
            .replace(&format!("//{inside}"), &format!("//{public}"))
            .replace(&format!("//127.0.0.1:{port}"), &format!("//{public}"));
        if let Ok(v) = HeaderValue::from_str(&fixed) {
            res.headers_mut().insert(header::LOCATION, v);
        }
    }
    res.map(Body::new)
}

fn page(status: StatusCode, message: &str) -> Response {
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>agentvm</title><body style=\"font:14px system-ui;background:#0e0f11;color:#eee;padding:40px\"><h1 style=\"font-size:16px\">agentvm</h1><p>{}</p>",
        message.replace('&', "&amp;").replace('<', "&lt;")
    );
    (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}
