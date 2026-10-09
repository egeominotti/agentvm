//! Tailscale: a VM can join the user's tailnet, to be reached and managed from it (Tailscale SSH,
//! its ports). The auth key is not here: it lives in the Keychain.

use serde::{Deserialize, Serialize};

/// Whether new VMs join the tailnet, and how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct TailscaleSettings {
    /// New VMs join the tailnet (each launch can still choose).
    pub enabled: bool,
    /// Tailscale SSH: the tailnet's policy decides who may open a shell in the VM.
    pub ssh: bool,
    /// ACL tags the VMs advertise (`tag:agentvm`); required with an OAuth client secret.
    pub tags: Vec<String>,
}

impl Default for TailscaleSettings {
    fn default() -> Self {
        TailscaleSettings { enabled: false, ssh: true, tags: Vec::new() }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TailscaleSettingsError {
    #[error("tags look like tag:name, in lowercase letters, digits and dashes")]
    Tag,
}

impl TailscaleSettings {
    pub fn validate(&self) -> Result<(), TailscaleSettingsError> {
        let valid = |t: &String| {
            t.strip_prefix("tag:").is_some_and(|name| {
                !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            })
        };
        if self.tags.iter().all(valid) { Ok(()) } else { Err(TailscaleSettingsError::Tag) }
    }
}

/// What a joining VM is told, in `tailscale-spec.json` (the key comes apart, in a file read once).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailscaleSpec {
    pub hostname: String,
    pub ssh: bool,
    pub tags: Vec<String>,
}

/// The VM on the tailnet, as its guest reported it: its name and addresses, or why it did not join.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Tailnet {
    /// MagicDNS name: `agentvm-demo-web-4f94.tail1234.ts.net`.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub ips: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// What a VM named `vm_name` is told to join with.
pub fn spec_for(vm_name: &str, settings: &TailscaleSettings) -> TailscaleSpec {
    TailscaleSpec { hostname: tailnet_hostname(vm_name), ssh: settings.ssh, tags: settings.tags.clone() }
}

/// The VM's name on the tailnet (and in MagicDNS): `agentvm-demo-web-4f94`, from its `vm_name`.
pub fn tailnet_hostname(vm_name: &str) -> String {
    let name: String = format!("agentvm-{vm_name}").chars().take(63).collect();
    name.trim_end_matches('-').to_owned()
}

/// An auth key (`tskey-auth-…`) or an OAuth client secret (`tskey-client-…`, which may carry
/// `?ephemeral=true&preauthorized=true`). One word: it is written to a file, never interpreted.
pub fn valid_auth_key(key: &str) -> bool {
    let body = key.strip_prefix("tskey-").unwrap_or_default();
    body.len() >= 8
        && key.len() <= 512
        && key.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'?' | b'=' | b'&'))
}
