//! Tailscale: the names VMs take on the tailnet, the keys and tags agentvm accepts.

use agentvm::domain::ids::TaskId;
use agentvm::domain::tailscale::{self, TailscaleSettings};

/// Every VM is `agent-<its id>`, a UUIDv7: unique, so two VMs never collide on the tailnet (nor
/// does Tailscale rename one "-1"), and a VM restored from a snapshot is a machine of its own.
#[test]
fn a_vm_joins_the_tailnet_under_its_own_unique_name() {
    let id = TaskId::parse("01a12130-a1d9-70bb-a1f6-54136ef18b6a").unwrap();
    let name = tailscale::tailnet_hostname(&id);
    assert_eq!(name, "agent-01a12130-a1d9-70bb-a1f6-54136ef18b6a");
    assert!(name.len() <= 63 && name.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'));
}

#[test]
fn only_tailscale_keys_are_accepted() {
    for ok in [
        "tskey-auth-kAbC123CNTRL-0123456789abcdefABCDEF",
        "tskey-client-kAbC123CNTRL-0123456789abcdef?ephemeral=true&preauthorized=true",
    ] {
        assert!(tailscale::valid_auth_key(ok), "{ok}");
    }
    for bad in ["", "tskey-", "sk-ant-oat01-x", "tskey-auth-abc def", "tskey-auth-abc\"; rm -rf /", "tskey-auth-abc\nx"]
    {
        assert!(!tailscale::valid_auth_key(bad), "{bad:?}");
    }
}

#[test]
fn tags_are_checked_as_tailscale_writes_them() {
    let s = |tags: &[&str]| TailscaleSettings {
        tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        ..Default::default()
    };
    assert_eq!(s(&["tag:agentvm", "tag:dev-box"]).validate(), Ok(()));
    assert_eq!(s(&[]).validate(), Ok(()));
    assert!(s(&["agentvm"]).validate().is_err());
    assert!(s(&["tag:Dev"]).validate().is_err());
    assert!(s(&["tag:a,b"]).validate().is_err());
    assert!(s(&["tag:"]).validate().is_err());
}

#[test]
fn off_until_asked_and_with_ssh_once_on() {
    let d = TailscaleSettings::default();
    assert!(!d.enabled);
    assert!(d.ssh, "a VM on the tailnet is managed over Tailscale SSH unless turned off");
    assert!(d.tags.is_empty());
}

/// What a joining VM is told: its tailnet name, SSH and tags as the settings say.
#[test]
fn a_joining_vm_is_told_its_name_and_options() {
    let settings = TailscaleSettings { enabled: true, ssh: false, tags: vec!["tag:agentvm".into()] };
    let spec = tailscale::spec_for(&TaskId::parse("01a12130-a1d9-70bb-a1f6-54136ef18b6a").unwrap(), &settings);
    assert_eq!(spec.hostname, "agent-01a12130-a1d9-70bb-a1f6-54136ef18b6a");
    assert!(!spec.ssh);
    assert_eq!(spec.tags, ["tag:agentvm"]);
    // The guest reads these three keys with jq.
    let json = serde_json::to_value(&spec).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"hostname": "agent-01a12130-a1d9-70bb-a1f6-54136ef18b6a", "ssh": false, "tags": ["tag:agentvm"]})
    );
}
