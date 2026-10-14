//! Host-only account orchestration; only connection metadata reaches clients.
mod access;
mod connections;
mod login;
use super::context::AppCtx;
pub use crate::domain::accounts::AccountAccess;
use crate::domain::accounts::{AccountKind, LoginStatus};
pub use access::{access, config};
pub use connections::{import, persist_refreshed, status};
pub use login::{begin_login, finish_login, login_status, register_login};
use std::collections::BTreeMap;
use std::sync::Mutex;

pub struct Accounts {
    pub(crate) gates: [tokio::sync::Mutex<()>; 5],
    pub(crate) control: Mutex<Control>,
    pub(crate) copilot: tokio::sync::Mutex<Option<CopilotCache>>,
    pub(crate) pending: Mutex<BTreeMap<AccountKind, (u64, crate::adapters::accounts::Session)>>,
}
pub(crate) struct CopilotCache {
    pub token: String,
    pub session: crate::adapters::accounts::Session,
    pub endpoint: String,
}
#[derive(Default)]
pub(crate) struct Control {
    pub generations: [u64; 5],
    pub login_generations: [u64; 5],
    pub logins: BTreeMap<String, (AccountKind, LoginStatus)>,
}
impl Default for Accounts {
    fn default() -> Self {
        Self {
            gates: std::array::from_fn(|_| Default::default()),
            control: Default::default(),
            copilot: Default::default(),
            pending: Default::default(),
        }
    }
}
pub(crate) fn index(account: AccountKind) -> usize {
    AccountKind::ALL.iter().position(|a| *a == account).expect("known account")
}
pub fn delete(ctx: &AppCtx, account: AccountKind) -> Result<(), String> {
    let mut control = ctx.accounts.control.lock().unwrap_or_else(|e| e.into_inner());
    if account == AccountKind::Claude {
        ctx.keychain.delete_claude_account().map_err(|_| "could not remove Claude account from Keychain")?;
    }
    ctx.keychain.delete_account(account).map_err(|_| "could not remove the account from Keychain")?;
    control.generations[index(account)] += 1;
    ctx.accounts.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&account);
    Ok(())
}
