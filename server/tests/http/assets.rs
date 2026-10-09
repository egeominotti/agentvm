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
