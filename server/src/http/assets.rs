//! The dashboard: the React app built in `web/dist`, embedded at build time (see `build.rs`).
//! Its page is at `/`, every other file at its path.

use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};

use super::error::ApiError;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));
}

pub async fn index() -> Result<Response, ApiError> {
    serve("index.html")
}

pub async fn file(Path(path): Path<String>) -> Result<Response, ApiError> {
    serve(&path)
}

/// The dashboard lived at `/next/` while it was being rebuilt: those links still lead to it.
pub async fn moved() -> Redirect {
    Redirect::permanent("/")
}

fn serve(path: &str) -> Result<Response, ApiError> {
    let body = lookup(path).ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "file not found"))?;
    let headers = [(header::CONTENT_TYPE, content_type(path)), (header::CACHE_CONTROL, cache_control(path))];
    Ok((headers, body).into_response())
}

/// The embedded file at `path`. Nothing for unknown files, nor for a path that tries to climb out
/// (`..`) or is not in the one spelling the table uses.
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

/// Vite names every file under `assets/` after its content, so it never changes: cached for
/// good. Fonts change with agentvm's releases only; the page is revalidated on every load, so a
/// new build shows up at once.
fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "max-age=31536000, immutable"
    } else if path.ends_with(".woff2") {
        "max-age=604800"
    } else {
        "no-cache"
    }
}
