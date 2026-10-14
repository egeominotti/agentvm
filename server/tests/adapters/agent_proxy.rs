use agentvm::adapters::agent_proxy::ProviderRelay;
use agentvm::adapters::keychain::Keychain;
use agentvm::app::supervisor::AppCtx;
use agentvm::domain::providers::{ApiProtocol, ProviderConfig};
use agentvm::secret::Secret;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use std::sync::Arc;

#[tokio::test]
async fn relay_streams_the_real_agentvm_api_and_removes_its_socket_on_drop() {
    let tmp = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = agentvm::config::Config {
        home: tmp.path().join("engine"),
        port,
        concurrency: 1,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 60,
        vm_helper: "agentvm-vm".into(),
        scripts_dir: "scripts".into(),
        min_free_mb: 0,
    };
    let ctx = Arc::new(AppCtx::new(config, Keychain::new(Some(tmp.path().join("none.keychain-db")))));
    let engine = tokio::spawn(async {
        axum::serve(listener, agentvm::http::router(ctx)).await.unwrap();
    });
    let path = tmp.path().join("relay.sock");
    let relay = ProviderRelay::start(
        &path,
        ProviderConfig {
            endpoint: format!("http://127.0.0.1:{port}/api"),
            protocol: ApiProtocol::Openai,
            requires_key: false,
        },
        Secret::new("a-host-test-credential".into()),
        false,
    )
    .unwrap();
    let stream = tokio::net::UnixStream::connect(&path).await.unwrap();
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);
    let response = sender
        .send_request(
            Request::get("/api/settings")
                .header("host", "guest")
                .header("authorization", "Bearer guest-dummy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(Body::new(response.into_body()), 1 << 20).await.unwrap();
    let settings: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(settings["settings"]["timeout_s"], 60);
    assert_eq!(settings["settings"]["model"], "default");
    assert!(settings["limits"]["cpus"].as_u64().unwrap() > 0);
    let response = sender
        .send_request(Request::get("/outside").header("host", "guest").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    drop(relay);
    assert!(!path.exists());
    engine.abort();
}
