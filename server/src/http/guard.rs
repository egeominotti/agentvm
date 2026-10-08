//! The gate every request goes through first. Proxied VM services leave here for their VM; the
//! rest must be the loopback dashboard itself (defense against DNS rebinding and cross-origin
//! requests, POST and WebSocket alike), and no other site may frame what it gets back.

use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::Ctx;
use super::error::ApiError;

pub async fn loopback_only(State(ctx): Ctx, req: Request, next: Next) -> Response {
    let port = ctx.config.port;
    // `<port>.<vm>.localhost`: a service inside a VM, never the dashboard or its API.
    let host = req.headers().get(header::HOST).and_then(|v| v.to_str().ok());
    if let Some((guest_port, vm)) = host.and_then(|h| crate::domain::hostname::parse_proxy_host(h, port)) {
        return super::proxy::forward(ctx.clone(), guest_port, vm, req).await;
    }
    if !is_loopback_request(port, &req) {
        return ApiError::new(StatusCode::FORBIDDEN, "request not allowed: use http://127.0.0.1").into_response();
    }
    let mut res = next.run(req).await;
    deny_framing(&mut res);
    res
}

fn is_loopback_request(port: u16, req: &Request) -> bool {
    let allowed = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let header = |name| req.headers().get(name).and_then(|v| v.to_str().ok());
    let host_ok = header(header::HOST).is_some_and(|h| allowed.iter().any(|a| a == h));
    let origin_ok = header(header::ORIGIN).is_none_or(|o| allowed.iter().any(|a| o == format!("http://{a}")));
    // Origin must be checked on GETs too: a cross-site WebSocket to a terminal is a GET.
    host_ok && origin_ok
}

/// No other site may show the dashboard in a frame and trick clicks on it.
fn deny_framing(res: &mut Response) {
    let h = res.headers_mut();
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("frame-ancestors 'none'"));
}
