//! Settings › Git access: tokens for private repositories, one per host. Their values never
//! come back out of the server.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::helpers::{app, json_req, send};

#[tokio::test]
async fn the_hosts_with_a_token_are_listed_never_the_tokens() {
    let home = tempfile::tempdir().unwrap();
    let req = Request::get("/api/settings/git").header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    let (status, body) = send(app(home.path()), req).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["hosts"], serde_json::json!([]), "{body}");
}

#[tokio::test]
async fn a_token_for_something_that_is_not_a_host_is_refused() {
    let home = tempfile::tempdir().unwrap();
    for host in ["github", "evil%20host", "GitHub.com"] {
        let req = json_req("PUT", &format!("/api/settings/git/{host}"), serde_json::json!({ "token": "github_pat_x" }));
        let (status, body) = send(app(home.path()), req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{host}: {body}");
    }
    let req = json_req("PUT", "/api/settings/git/github.com", serde_json::json!({ "token": "two words" }));
    let (status, body) = send(app(home.path()), req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
