//! The dashboard's files. `index.html` is the page at `/`; every other file under `web/` is
//! embedded at build time (see `build.rs`) and served at `/assets/<path under web/>`. Third-party
//! files are also at `/vendor/<file>`, and the logo at `/logo.svg`.

use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};

use super::error::ApiError;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));
}

pub async fn dashboard() -> Html<&'static str> {
    Html(include_str!("web/index.html"))
}

pub async fn asset(Path(path): Path<String>) -> Result<Response, ApiError> {
    serve(&path)
}

pub async fn vendor(Path(path): Path<String>) -> Result<Response, ApiError> {
    serve(&format!("vendor/{path}"))
}

pub async fn logo() -> Result<Response, ApiError> {
    serve("logo.svg")
}

fn serve(path: &str) -> Result<Response, ApiError> {
    let body = lookup(path).ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "file not found"))?;
    let headers = [(header::CONTENT_TYPE, content_type(path)), (header::CACHE_CONTROL, cache_control(path))];
    Ok((headers, body).into_response())
}

/// The embedded `web/<path>`. Nothing for unknown files, nor for a path that tries to climb out
/// of `web/` (`..`) or is not in the one spelling the table uses.
fn lookup(path: &str) -> Option<&'static [u8]> {
    if path.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return None;
    }
    let files = embedded::FILES;
    files.binary_search_by(|(name, _)| (*name).cmp(path)).ok().map(|i| files[i].1)
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map_or("", |(_, ext)| ext) {
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "html" => "text/html; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Fonts never change; third-party code and the logo rarely do. The dashboard's own code is
/// revalidated on every load, so a new build shows up at once.
fn cache_control(path: &str) -> &'static str {
    if path.ends_with(".woff2") {
        "max-age=604800"
    } else if path.starts_with("vendor/") || path == "logo.svg" {
        "max-age=86400"
    } else {
        "no-cache"
    }
}
