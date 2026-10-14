use crate::helpers::ctx;
use agentvm::adapters::accounts::{Connection, Reference, Session};
use agentvm::adapters::keychain::Keychain;
use agentvm::app::accounts::{access, delete, status};
use agentvm::domain::accounts::AccountKind;
use agentvm::secret::Secret;
use std::process::Command;

struct Cleanup(std::path::PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
    }
}
fn setup(home: &std::path::Path) -> (std::sync::Arc<agentvm::app::context::AppCtx>, Cleanup) {
    let path = Cleanup(home.join("accounts.keychain-db"));
    assert!(Command::new("security").args(["create-keychain", "-p", "x"]).arg(&path.0).status().unwrap().success());
    let mut ctx = ctx(home);
    std::sync::Arc::get_mut(&mut ctx).unwrap().keychain = Keychain::new(Some(path.0.clone()));
    (ctx, path)
}
fn save(ctx: &agentvm::app::context::AppCtx, account: AccountKind, connection: Connection) {
    ctx.keychain.write_account(account, &Secret::new(serde_json::to_string(&connection).unwrap())).unwrap();
}

#[tokio::test]
async fn kilo_native_file_reference_preserves_organization_through_keychain_access() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, _cleanup) = setup(home.path());
    let path = home.path().join("native-kilo-auth.json");
    let text =
        r#"{"kilo":{"type":"oauth","access":"host-org-token","refresh":"","expires":0,"accountId":"org-selected"}}"#;
    std::fs::write(&path, text).unwrap();
    save(&ctx, AccountKind::Kilo, Connection { session: None, reference: Some(Reference::File(path.clone())) });
    assert!(status(&ctx, AccountKind::Kilo).connected);
    let result = access(&ctx, AccountKind::Kilo, false).await.unwrap();
    assert_eq!(result.credential.expose(), "host-org-token");
    assert_eq!(result.headers, vec![("X-KILOCODE-ORGANIZATIONID".into(), "org-selected".into())]);
    assert!(result.catalog_credential.is_none());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    std::fs::write(&path, r#"{"kilo":{"type":"api","key":"host-personal-token"}}"#).unwrap();
    let result = access(&ctx, AccountKind::Kilo, false).await.unwrap();
    assert_eq!(result.credential.expose(), "host-personal-token");
    assert!(result.headers.is_empty());
}

#[tokio::test]
async fn referenced_codex_access_reloads_and_never_owns_refresh() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, _cleanup) = setup(home.path());
    let path = home.path().join("native-auth.json");
    let write = |access: &str, expires: u64| {
        std::fs::write(&path,serde_json::json!({"auth_mode":"chatgpt","tokens":{"access_token":access,"refresh_token":"never-rotate-this","account_id":"workspace","expires_at":expires}}).to_string()).unwrap();
    };
    write("first-host-access", 2000000000);
    save(&ctx, AccountKind::Chatgpt, Connection { session: None, reference: Some(Reference::File(path.clone())) });
    assert!(status(&ctx, AccountKind::Chatgpt).connected);
    assert_eq!(access(&ctx, AccountKind::Chatgpt, false).await.unwrap().credential.expose(), "first-host-access");
    write("second-host-access", 2000000000);
    let before = std::fs::read(&path).unwrap();
    let second = access(&ctx, AccountKind::Chatgpt, true).await.unwrap();
    assert_eq!(second.credential.expose(), "second-host-access");
    assert_eq!(second.headers, vec![("ChatGPT-Account-ID".into(), "workspace".into())]);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    write("expired-host-access", 1);
    assert!(!status(&ctx, AccountKind::Chatgpt).connected);
    let error = access(&ctx, AccountKind::Chatgpt, true).await.err().unwrap();
    assert!(error.contains("sign in on the host"));
    assert!(!error.contains("never-rotate-this"));
    delete(&ctx, AccountKind::Chatgpt).unwrap();
    assert!(ctx.keychain.read_account(AccountKind::Chatgpt).unwrap().is_none());
    assert!(path.exists());
}

#[test]
fn owned_account_storage_and_status_never_expose_credentials() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, _cleanup) = setup(home.path());
    let token = "host-only-opaque-keychain-token";
    save(
        &ctx,
        AccountKind::Kilo,
        Connection {
            session: Some(Session {
                access_token: token.into(),
                refresh_token: None,
                account_id: None,
                expires_at: 2000000000,
            }),
            reference: None,
        },
    );
    let body = serde_json::to_string(&status(&ctx, AccountKind::Kilo)).unwrap();
    assert!(!body.contains(token));
    assert!(status(&ctx, AccountKind::Kilo).connected);
    delete(&ctx, AccountKind::Kilo).unwrap();
    delete(&ctx, AccountKind::Kilo).unwrap();
    assert!(!status(&ctx, AccountKind::Kilo).connected);
    assert!(!home.path().join("settings.json").exists());
}

#[tokio::test]
async fn malformed_saved_account_is_not_advertised_or_returned() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, _cleanup) = setup(home.path());
    save(
        &ctx,
        AccountKind::Kilo,
        Connection {
            session: Some(Session {
                access_token: "bad\nheader".into(),
                refresh_token: None,
                account_id: None,
                expires_at: 2000000000,
            }),
            reference: None,
        },
    );
    assert!(!status(&ctx, AccountKind::Kilo).connected);
    let error = access(&ctx, AccountKind::Kilo, false).await.err().unwrap();
    assert!(!error.contains("bad\nheader"));
}

#[test]
fn removing_claude_account_removes_the_existing_saved_token() {
    let home = tempfile::tempdir().unwrap();
    let (ctx, _cleanup) = setup(home.path());
    ctx.keychain.write_token(&Secret::new("sk-ant-existing-account".into())).unwrap();
    assert!(status(&ctx, AccountKind::Claude).connected);
    delete(&ctx, AccountKind::Claude).unwrap();
    assert!(!status(&ctx, AccountKind::Claude).connected);
}

#[tokio::test]
async fn rotated_session_survives_a_real_keychain_write_failure_in_host_memory() {
    use agentvm::app::accounts::persist_refreshed;
    let home = tempfile::tempdir().unwrap();
    let (ctx, cleanup) = setup(home.path());
    let session = |token: &str| Session {
        access_token: token.into(),
        refresh_token: Some("rotating-refresh".into()),
        account_id: Some("workspace".into()),
        expires_at: 2000000000,
    };
    save(&ctx, AccountKind::Chatgpt, Connection { session: Some(session("old-access")), reference: None });
    assert!(Command::new("chflags").arg("uchg").arg(&cleanup.0).status().unwrap().success());
    let failed = persist_refreshed(&ctx, AccountKind::Chatgpt, 0, session("new-access"));
    assert!(Command::new("chflags").arg("nouchg").arg(&cleanup.0).status().unwrap().success());
    assert!(failed.is_err());
    assert_eq!(access(&ctx, AccountKind::Chatgpt, false).await.unwrap().credential.expose(), "new-access");
    let stored = ctx.keychain.read_account(AccountKind::Chatgpt).unwrap().unwrap();
    assert!(stored.expose().contains("new-access"));
    delete(&ctx, AccountKind::Chatgpt).unwrap();
    assert!(access(&ctx, AccountKind::Chatgpt, false).await.is_err());
}

#[tokio::test]
async fn pending_login_does_not_discard_an_existing_rotated_session() {
    use agentvm::app::accounts::{finish_login, persist_refreshed, register_login};
    use agentvm::domain::accounts::LoginStart;
    let home = tempfile::tempdir().unwrap();
    let (ctx, _cleanup) = setup(home.path());
    let session = |token: &str| Session {
        access_token: token.into(),
        refresh_token: Some("refresh".into()),
        account_id: Some("workspace".into()),
        expires_at: 2000000000,
    };
    save(&ctx, AccountKind::Chatgpt, Connection { session: Some(session("old")), reference: None });
    let start = LoginStart {
        id: "pending-replacement".into(),
        verification_url: "https://auth.openai.com/codex/device".into(),
        user_code: None,
        expires_in: 900,
    };
    let ticket = register_login(&ctx, AccountKind::Chatgpt, &start).unwrap();
    persist_refreshed(&ctx, AccountKind::Chatgpt, 0, session("rotated-before-replacement")).unwrap();
    assert_eq!(
        access(&ctx, AccountKind::Chatgpt, false).await.unwrap().credential.expose(),
        "rotated-before-replacement"
    );
    delete(&ctx, AccountKind::Chatgpt).unwrap();
    assert!(persist_refreshed(&ctx, AccountKind::Chatgpt, 0, session("must-not-resurrect")).is_err());
    assert!(finish_login(&ctx, AccountKind::Chatgpt, &start.id, ticket, Ok(session("stale-replacement"))).is_err());
    assert!(ctx.keychain.read_account(AccountKind::Chatgpt).unwrap().is_none());
}
