//! The dashboard: the React app built in `web/dist`, embedded at build time (see `build.rs`).
//! Its page is at `/`, every other file at its path.

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};

use super::error::ApiError;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));
}

pub async fn index(headers: HeaderMap) -> Result<Response, ApiError> {
    serve("index.html", &headers)
}

pub async fn file(Path(path): Path<String>, headers: HeaderMap) -> Result<Response, ApiError> {
    serve(&path, &headers)
}

/// The dashboard lived at `/next/` while it was being rebuilt: those links still lead to it.
pub async fn moved() -> Redirect {
    Redirect::permanent("/")
}

fn serve(path: &str, request: &HeaderMap) -> Result<Response, ApiError> {
    let (etag, body) = lookup(path).ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "file not found"))?;
    let headers = [
        (header::CONTENT_TYPE, content_type(path)),
        (header::CACHE_CONTROL, cache_control(path)),
        (header::ETAG, etag),
    ];
    if fresh(request, etag) {
        return Ok((StatusCode::NOT_MODIFIED, headers).into_response());
    }
    Ok((headers, body).into_response())
}

/// Whether the browser already holds this version (`If-None-Match` lists its ETag, or `*`).
fn fresh(request: &HeaderMap, etag: &str) -> bool {
    let Some(held) = request.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    held.split(',').map(str::trim).any(|tag| tag == "*" || tag.trim_start_matches("W/") == etag)
}

/// The embedded file at `path`, with its ETag. Nothing for unknown files, nor for a path that
/// tries to climb out (`..`) or is not in the one spelling the table uses.
fn lookup(path: &str) -> Option<(&'static str, &'static [u8])> {
    if path.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return None;
    }
    let files = embedded::FILES;
    files.binary_search_by(|(name, _, _)| (*name).cmp(path)).ok().map(|i| (files[i].1, files[i].2))
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
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Vite names every file under `assets/` after its content, so it never changes: cached for
/// good. Everything else (the page, fonts) is revalidated on every load by its ETag: a 304 when
/// unchanged, and a new build shows up at once.
fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") { "max-age=31536000, immutable" } else { "no-cache" }
}
