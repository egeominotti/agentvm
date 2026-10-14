use agentvm::adapters::agent_proxy::{AccountRefresh, ProviderRelay};
use agentvm::adapters::keychain::Keychain;
use agentvm::app::supervisor::AppCtx;
use agentvm::domain::accounts::AccountKind;
use agentvm::domain::providers::{ApiProtocol, ProviderConfig};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use std::sync::Arc;

#[tokio::test]
async fn account_relay_uses_host_connection_status_and_never_accepts_guest_credentials() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = Arc::new(AppCtx::new(
        agentvm::config::Config {
            home: tmp.path().join("engine"),
            port: 0,
            concurrency: 1,
            cpus: 2,
            memory_mb: 2048,
            timeout_s: 60,
            vm_helper: "agentvm-vm".into(),
            scripts_dir: "scripts".into(),
            min_free_mb: 0,
        },
        Keychain::new(Some(tmp.path().join("no-account.keychain-db"))),
    ));
    let refresh: AccountRefresh = Arc::new(move |force| {
        let ctx = ctx.clone();
        Box::pin(async move { agentvm::app::accounts::access(&ctx, AccountKind::Chatgpt, force).await })
    });
    let path = tmp.path().join("account.sock");
    let relay = ProviderRelay::start_account(
        &path,
        AccountKind::Chatgpt,
        ProviderConfig {
            endpoint: "https://chatgpt.com/backend-api/codex".into(),
            protocol: ApiProtocol::Openai,
            requires_key: true,
        },
        refresh,
    )
    .unwrap();
    let stream = tokio::net::UnixStream::connect(&path).await.unwrap();
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);
    let response = sender
        .send_request(
            Request::get("/backend-api/codex/models?client_version=0.162.1")
                .header("host", "guest")
                .header("authorization", "Bearer guest-token-is-not-a-host-login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = axum::body::to_bytes(Body::new(response.into_body()), 1 << 20).await.unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!body.contains("guest-token-is-not-a-host-login"));
    assert!(!body.contains("refresh_token"));
    drop(relay);
    assert!(!path.exists());
}
