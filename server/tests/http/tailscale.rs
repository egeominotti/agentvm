//! Settings › Tailscale: the auth key VMs join the user's tailnet with. Its value never comes back
//! out of the server.

use axum::body::Body;
use axum::http::{Request, StatusCode};

use crate::helpers::{app, json_req, send};

#[tokio::test]
async fn whether_a_key_is_saved_is_said_never_the_key() {
    let home = tempfile::tempdir().unwrap();
    let req = Request::get("/api/settings/tailscale").header("host", "127.0.0.1:7777").body(Body::empty()).unwrap();
    let (status, body) = send(app(home.path()), req).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, serde_json::json!({ "key_saved": false }));
}

#[tokio::test]
async fn something_that_is_not_a_tailscale_key_is_refused() {
    let home = tempfile::tempdir().unwrap();
    for key in ["sk-ant-oat01-x", "tskey-auth-two words", ""] {
        let req = json_req("PUT", "/api/settings/tailscale", serde_json::json!({ "key": key }));
        let (status, body) = send(app(home.path()), req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{key:?}: {body}");
        assert!(body.to_string().contains("tskey-"), "{body}");
    }
}

#[tokio::test]
async fn removing_a_key_that_is_not_there_is_fine() {
    let home = tempfile::tempdir().unwrap();
    let req = Request::delete("/api/settings/tailscale")
        .header("host", "127.0.0.1:7777")
        .header("origin", "http://127.0.0.1:7777")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app(home.path()), req).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
}

/// Joining from a VM's page: an unknown machine is 404, not a silent success.
#[tokio::test]
async fn joining_an_unknown_machine_is_not_found() {
    let home = tempfile::tempdir().unwrap();
    for method in ["POST", "DELETE"] {
        let req = json_req(method, "/api/tasks/20261008-185855-4f94/tailscale", serde_json::json!({}));
        let (status, body) = send(app(home.path()), req).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method}: {body}");
    }
}
