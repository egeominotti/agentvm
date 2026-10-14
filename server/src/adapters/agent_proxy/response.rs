use crate::domain::agent_proxy::{StreamRedactor, response_header};
use crate::domain::agents::AgentKind;
use crate::domain::providers::ProviderConfig;
use crate::secret::Secret;
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures::StreamExt;

pub(super) fn catalog_json(catalog: serde_json::Value, key: &Secret) -> Response {
    let body = key.redact(&catalog.to_string());
    Response::builder()
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| error(StatusCode::BAD_GATEWAY, "invalid model catalog"))
}

pub(super) async fn model_catalog(
    upstream: reqwest::Response,
    key: Secret,
    config: ProviderConfig,
    kind: AgentKind,
) -> Response {
    let read = async {
        let mut stream = upstream.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "model catalog interrupted")?;
            if bytes.len() + chunk.len() > 8 << 20 {
                return Err("model catalog exceeds 8 MiB");
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| "invalid provider model catalog")
    };
    let payload = match tokio::time::timeout(std::time::Duration::from_secs(20), read).await {
        Ok(Ok(payload)) => payload,
        Ok(Err(message)) => return error(StatusCode::BAD_GATEWAY, message),
        Err(_) => return error(StatusCode::GATEWAY_TIMEOUT, "model catalog timed out"),
    };
    match crate::domain::provider_models::filter_catalog(&config, kind, payload) {
        Some(catalog) => catalog_json(catalog, &key),
        None => error(StatusCode::BAD_GATEWAY, "invalid provider model catalog"),
    }
}

pub(super) fn stream(upstream: reqwest::Response, key: Secret) -> Response {
    let mut response = Response::builder().status(upstream.status());
    for (name, value) in upstream.headers() {
        if response_header(name.as_str())
            && let Ok(text) = value.to_str()
        {
            response = response.header(name, key.redact(text));
        }
    }
    let stream = upstream.bytes_stream();
    let redacted = futures::stream::unfold(
        (Box::pin(stream), StreamRedactor::new(key.expose().as_bytes().to_vec()), false),
        |(mut stream, mut redactor, done)| async move {
            if done {
                return None;
            }
            loop {
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        let output = redactor.push(&bytes);
                        if !output.is_empty() {
                            return Some((Ok::<_, std::io::Error>(output), (stream, redactor, false)));
                        }
                    }
                    Some(Err(_)) => {
                        return Some((
                            Err(std::io::Error::other("inference stream interrupted")),
                            (stream, redactor, true),
                        ));
                    }
                    None => {
                        let output = redactor.finish();
                        return (!output.is_empty()).then_some((Ok(output), (stream, redactor, true)));
                    }
                }
            }
        },
    );
    response
        .body(Body::from_stream(redacted))
        .unwrap_or_else(|_| error(StatusCode::BAD_GATEWAY, "invalid inference response"))
}

pub(super) fn error(status: StatusCode, message: &str) -> Response {
    (status, axum::Json(serde_json::json!({"error":{"message":message}}))).into_response()
}
