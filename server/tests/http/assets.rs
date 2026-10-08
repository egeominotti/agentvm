//! The dashboard's files, embedded and served.

use axum::http::StatusCode;

use crate::helpers::*;

/// Every file of the dashboard is embedded and served with its type, without listing it in Rust.
#[tokio::test]
async fn every_dashboard_file_is_served_with_its_type() {
    let web = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http/web");
    let mut files = vec![];
    let mut dirs = vec![web.clone()];
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                dirs.push(path);
            } else if !e.file_name().to_string_lossy().starts_with('.') && path != web.join("index.html") {
                files.push(path.strip_prefix(&web).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    assert!(files.iter().any(|f| f.ends_with(".js")) && files.iter().any(|f| f.ends_with(".css")));
    for f in &files {
        let res = fetch(&format!("/assets/{f}")).await;
        assert_eq!(res.status(), StatusCode::OK, "{f}");
        let ty = res.headers()["content-type"].to_str().unwrap().to_owned();
        let want = match f.rsplit('.').next().unwrap() {
            "js" => "text/javascript",
            "css" => "text/css",
            "woff2" => "font/woff2",
            "svg" => "image/svg+xml",
            _ => continue,
        };
        assert!(ty.starts_with(want), "{f}: {ty}");
    }
    assert_eq!(fetch("/vendor/xterm.css").await.status(), StatusCode::OK);
    assert_eq!(fetch("/logo.svg").await.status(), StatusCode::OK);
    assert_eq!(fetch("/").await.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_or_escaping_asset_paths_are_not_found() {
    for path in ["/assets/nope.js", "/assets/js/../index.html", "/assets/../Cargo.toml", "/vendor/../../build.rs"] {
        assert_eq!(fetch(path).await.status(), StatusCode::NOT_FOUND, "{path}");
    }
}
