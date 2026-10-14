//! A per-VM Unix HTTP relay with host-only provider or account authentication.
use crate::domain::accounts::{AccountAccess, AccountKind};
use crate::domain::agents::AgentKind;
use crate::domain::providers::ProviderConfig;
use crate::secret::Secret;
use futures::future::BoxFuture;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::UnixListener;

mod forward;
mod response;

pub type AccountRefresh = Arc<dyn Fn(bool) -> BoxFuture<'static, Result<AccountAccess, String>> + Send + Sync>;

enum Auth {
    Api { key: Secret, oauth: bool },
    Account { account: AccountKind, refresh: AccountRefresh },
}
struct Upstream {
    client: reqwest::Client,
    config: ProviderConfig,
    auth: Auth,
    context: Option<(AgentKind, String)>,
}
pub struct ProviderRelay {
    path: PathBuf,
    task: tokio::task::JoinHandle<()>,
}

impl ProviderRelay {
    pub fn start(path: &Path, config: ProviderConfig, key: Secret, oauth: bool) -> Result<Self, String> {
        Self::bind(path, config, Auth::Api { key, oauth }, None)
    }
    pub fn start_for_agent(
        path: &Path,
        config: ProviderConfig,
        key: Secret,
        oauth: bool,
        kind: AgentKind,
        session: String,
    ) -> Result<Self, String> {
        Self::bind(path, config, Auth::Api { key, oauth }, Some((kind, session)))
    }
    pub fn start_account(
        path: &Path,
        account: AccountKind,
        config: ProviderConfig,
        refresh: AccountRefresh,
    ) -> Result<Self, String> {
        Self::bind(path, config, Auth::Account { account, refresh }, None)
    }
    fn bind(
        path: &Path,
        config: ProviderConfig,
        auth: Auth,
        context: Option<(AgentKind, String)>,
    ) -> Result<Self, String> {
        config.validate()?;
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        let listener = UnixListener::bind(path).map_err(|e| e.to_string())?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let state = Arc::new(Upstream { client, config, auth, context });
        let router = axum::Router::new().fallback(forward::forward).with_state(state);
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(Self { path: path.to_owned(), task })
    }
}
impl Drop for ProviderRelay {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}
