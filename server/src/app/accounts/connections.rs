use super::AppCtx;
use crate::adapters::accounts::{self, Connection, Reference, Session};
use crate::domain::accounts::{AccountKind, AccountStatus};
use crate::secret::Secret;
use std::path::PathBuf;

pub(crate) fn load(ctx: &AppCtx, account: AccountKind) -> Result<Connection, String> {
    let value = ctx
        .keychain
        .read_account(account)
        .map_err(|_| "account Keychain unavailable")?
        .ok_or("account not connected; import a local login or sign in")?;
    serde_json::from_str(value.expose()).map_err(|_| "saved account connection is malformed; reconnect".into())
}
pub(crate) fn save(ctx: &AppCtx, account: AccountKind, connection: &Connection) -> Result<(), String> {
    let serialized = serde_json::to_string(connection).map_err(|_| "cannot encode account session")?;
    ctx.keychain
        .write_account(account, &Secret::new(serialized))
        .map_err(|_| "could not save account in Keychain".into())
}
/// Rotation has already happened upstream. Retain its result on the host if Keychain is temporarily unwritable.
pub fn persist_refreshed(ctx: &AppCtx, account: AccountKind, generation: u64, session: Session) -> Result<(), String> {
    let control = ctx.accounts.control.lock().unwrap_or_else(|e| e.into_inner());
    if control.generations[super::index(account)] != generation {
        return Err("account connection changed during refresh; retry".into());
    }
    ctx.accounts.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(account, (generation, session.clone()));
    save(ctx, account, &Connection { session: Some(session), reference: None })?;
    ctx.accounts.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&account);
    Ok(())
}
pub(crate) fn read(ctx: &AppCtx, account: AccountKind, connection: &Connection) -> Result<Session, String> {
    if let Some(reference) = &connection.reference {
        return match reference {
            Reference::File(path) => accounts::read_reference(account, path),
            Reference::Keychain { service, account: entry } => {
                let secret = ctx
                    .keychain
                    .read_cli_account(service, entry.as_deref())
                    .map_err(|_| "native account Keychain unavailable")?
                    .ok_or("native CLI login missing; sign in on the host again")?;
                let value = serde_json::from_str(secret.expose())
                    .unwrap_or_else(|_| serde_json::json!({"token":secret.expose()}));
                accounts::parse_session(account, &value)
            }
        };
    }
    let session = connection.session.as_ref().ok_or("account session missing; reconnect")?;
    let value = serde_json::to_value(session).map_err(|_| "saved account session malformed")?;
    accounts::parse_session(account, &value)
}
pub fn status(ctx: &AppCtx, account: AccountKind) -> AccountStatus {
    let result = if account == AccountKind::Claude {
        ctx.keychain
            .read_token()
            .map(|_| "Claude token setting".to_owned())
            .map_err(|_| "Claude account token missing; use claude setup-token".into())
    } else {
        load(ctx, account).and_then(|connection| {
            let session = read(ctx, account, &connection)?;
            let expired = session.expires_at != 0 && session.expires_at <= accounts::now();
            if expired
                && (connection.reference.is_some()
                    || session.refresh_token.is_none()
                    || !matches!(account, AccountKind::Chatgpt | AccountKind::Google))
            {
                return Err("account session expired; sign in on the host again".into());
            }
            Ok(if connection.reference.is_some() { "Local CLI login" } else { "AgentVM sign-in" }.into())
        })
    };
    let (connected, source, error) = match result {
        Ok(source) => (true, Some(source), None),
        Err(error) => (false, None, Some(error)),
    };
    AccountStatus {
        id: account,
        name: account.name().into(),
        connected,
        source,
        error,
        can_import: account != AccountKind::Claude,
        can_login: account != AccountKind::Claude,
    }
}
fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| "host home directory unavailable".into())
}
pub fn import(ctx: &AppCtx, account: AccountKind) -> Result<AccountStatus, String> {
    if account == AccountKind::Claude {
        return Err(
            "Claude account is connected through the Claude token setting; native CLI import is unavailable".into()
        );
    }
    let home = home()?;
    let reference = match account {
        AccountKind::Chatgpt => {
            let codex = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
            let auth = codex.join("auth.json");
            if auth.exists() {
                Reference::File(auth)
            } else {
                use sha2::{Digest, Sha256};
                let canonical = codex.canonicalize().unwrap_or(codex);
                let hash = format!("{:x}", Sha256::digest(canonical.to_string_lossy().as_bytes()));
                Reference::Keychain { service: "Codex Auth".into(), account: Some(format!("cli|{}", &hash[..16])) }
            }
        }
        AccountKind::Google => {
            let path = home.join(".gemini/oauth_creds.json");
            if path.exists() {
                Reference::File(path)
            } else {
                Reference::Keychain { service: "gemini-cli-oauth".into(), account: Some("main-account".into()) }
            }
        }
        AccountKind::Copilot => Reference::File(
            std::env::var_os("COPILOT_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".copilot"))
                .join("config.json"),
        ),
        AccountKind::Kilo => Reference::File(home.join(".local/share/kilo/auth.json")),
        AccountKind::Claude => unreachable!(),
    };
    let mut candidates = if account == AccountKind::Copilot {
        let Reference::File(path) = reference else { unreachable!() };
        accounts::copilot_references(&path)?
    } else {
        vec![reference]
    };
    if account == AccountKind::Chatgpt {
        candidates.push(Reference::File(home.join(".local/share/opencode/auth.json")));
    }
    let mut found = None;
    let mut error = "local CLI login missing; sign in on the host first".to_owned();
    for reference in candidates {
        let connection = Connection { session: None, reference: Some(reference) };
        match read(ctx, account, &connection) {
            Ok(session) => {
                found = Some((connection, session));
                break;
            }
            Err(e) => error = e,
        }
    }
    let (mut connection, session) = found.ok_or(error)?;
    if session.expires_at != 0
        && session.expires_at <= accounts::now()
        && (account != AccountKind::Google || session.refresh_token.is_none())
    {
        return Err("local CLI login expired; sign in on the host again".into());
    }
    // Google's installed-app refresh token is stable; Codex rotating tokens stay referenced.
    if account == AccountKind::Google {
        connection = Connection { session: Some(session), reference: None };
    }
    {
        let mut control = ctx.accounts.control.lock().unwrap_or_else(|e| e.into_inner());
        save(ctx, account, &connection)?;
        control.generations[super::index(account)] += 1;
    }
    let result = status(ctx, account);
    if !result.connected {
        return Err(result.error.unwrap_or_else(|| "account unavailable".into()));
    }
    Ok(result)
}
