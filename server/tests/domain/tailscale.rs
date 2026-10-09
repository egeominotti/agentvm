//! Tailscale: the names VMs take on the tailnet, the keys and tags agentvm accepts.

use agentvm::domain::tailscale::{self, TailscaleSettings};

#[test]
fn a_vm_joins_the_tailnet_under_its_own_name() {
    assert_eq!(tailscale::tailnet_hostname("demo-web-4f94"), "agentvm-demo-web-4f94");
    // Always a valid DNS label, whatever the VM's name.
    let long = tailscale::tailnet_hostname(&"x".repeat(100));
    assert!(long.len() <= 63 && !long.ends_with('-'), "{long}");
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
    let spec = tailscale::spec_for("demo-web-4f94", &settings);
    assert_eq!(spec.hostname, "agentvm-demo-web-4f94");
    assert!(!spec.ssh);
    assert_eq!(spec.tags, ["tag:agentvm"]);
    // The guest reads these three keys with jq.
    let json = serde_json::to_value(&spec).unwrap();
    assert_eq!(json, serde_json::json!({"hostname": "agentvm-demo-web-4f94", "ssh": false, "tags": ["tag:agentvm"]}));
}
