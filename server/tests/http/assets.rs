//! The dashboard (`web/dist`, built by `bun run build`), embedded and served.

use axum::http::StatusCode;

use crate::helpers::*;

fn dist() -> std::path::PathBuf {
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/dist");
    assert!(dist.join("index.html").is_file(), "build the dashboard first: cd web && bun install && bun run build");
    dist
}

/// Every built file is served at its path with its type; the page itself at `/`.
#[tokio::test]
async fn every_dashboard_file_is_served_with_its_type() {
    let dist = dist();
    let page = fetch("/").await;
    assert_eq!(page.status(), StatusCode::OK);
    assert!(page.headers()["content-type"].to_str().unwrap().starts_with("text/html"));
    assert_eq!(page.headers()["cache-control"], "no-cache");
    let mut dirs = vec![dist.clone()];
    let (mut js, mut css) = (0, 0);
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let f = path.strip_prefix(&dist).unwrap().to_string_lossy().into_owned();
            let res = fetch(&format!("/{f}")).await;
            assert_eq!(res.status(), StatusCode::OK, "{f}");
            let ty = res.headers()["content-type"].to_str().unwrap().to_owned();
            let want = match f.rsplit('.').next().unwrap() {
                "js" => {
                    js += 1;
                    "text/javascript"
                }
                "css" => {
                    css += 1;
                    "text/css"
                }
                "woff2" => "font/woff2",
                "ttf" => "font/ttf",
                "svg" => "image/svg+xml",
                _ => continue,
            };
            assert!(ty.starts_with(want), "{f}: {ty}");
        }
    }
    assert!(js > 0 && css > 0, "the build has no scripts or styles");
}

/// Vite names every file under `assets/` after its content: it never changes.
#[tokio::test]
async fn hashed_files_are_cached_for_good() {
    for e in std::fs::read_dir(dist().join("assets")).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let res = fetch(&format!("/assets/{name}")).await;
        assert_eq!(res.headers()["cache-control"], "max-age=31536000, immutable", "{name}");
    }
}

/// The page and the fonts (5 MB, not named after their content) are checked on every load, and a
/// file that did not change costs a 304 instead of its bytes; a new build shows up at once.
#[tokio::test]
async fn unhashed_files_are_revalidated_by_their_etag() {
    for path in ["/", "/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"] {
        let first = fetch(path).await;
        assert_eq!(first.status(), StatusCode::OK, "{path}");
        assert_eq!(first.headers()["cache-control"], "no-cache", "{path}");
        let etag = first.headers().get("etag").unwrap_or_else(|| panic!("{path} has no ETag")).clone();
        assert!(etag.to_str().unwrap().starts_with('"'), "{path}: a strong ETag is quoted");

        let again = fetch_with(path, "if-none-match", etag.to_str().unwrap()).await;
        assert_eq!(again.status(), StatusCode::NOT_MODIFIED, "{path}");
        assert_eq!(again.headers()["etag"], etag, "{path}");
        let body = axum::body::to_bytes(again.into_body(), 1 << 10).await.unwrap();
        assert!(body.is_empty(), "{path}: a 304 has no body");

        let changed = fetch_with(path, "if-none-match", "\"older\"").await;
        assert_eq!(changed.status(), StatusCode::OK, "{path}");
    }
}

/// Two different files never share an ETag (it is the content's).
#[tokio::test]
async fn etags_follow_the_content() {
    let a = fetch("/fonts/JetBrainsMonoNerdFontMono-Regular.ttf").await;
    let b = fetch("/fonts/JetBrainsMonoNerdFontMono-Bold.ttf").await;
    assert_ne!(a.headers()["etag"], b.headers()["etag"]);
    assert_eq!(fetch("/").await.headers()["etag"], fetch("/").await.headers()["etag"]);
}

#[tokio::test]
async fn unknown_or_escaping_paths_are_not_found() {
    for path in ["/nope.js", "/assets/nope.js", "/assets/../index.html", "/../Cargo.toml", "/fonts/../../build.rs"] {
        assert_eq!(fetch(path).await.status(), StatusCode::NOT_FOUND, "{path}");
    }
}

/// The dashboard lived at /next/ while it was being rebuilt: those links still work.
#[tokio::test]
async fn the_old_next_address_leads_to_the_dashboard() {
    for path in ["/next", "/next/"] {
        let res = fetch(path).await;
        assert_eq!(res.status(), StatusCode::PERMANENT_REDIRECT, "{path}");
        assert_eq!(res.headers()["location"], "/", "{path}");
    }
}
