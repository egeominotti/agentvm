//! Settings, models, Claude Code versions and the limits of the Mac.

use agentvm::domain::settings::{HostLimits, Model, Settings, SettingsError};

fn limits() -> HostLimits {
    HostLimits { cpus: 18, ram_mb: 65536 }
}

#[test]
fn default_settings_are_valid() {
    let s = Settings {
        max_vms: 4,
        cpus: 4,
        memory_mb: 4096,
        timeout_s: 1800,
        model: Model::default_choice(),
        default_repo: None,
        claude_version: Default::default(),
        s3: None,
        auto_snapshots: Default::default(),
        tailscale: Default::default(),
    };
    assert!(s.validate(&limits()).is_ok());
}

/// Tags that Tailscale would refuse are refused when saving, not at the next launch.
#[test]
fn settings_reject_tailscale_tags_tailscale_would_refuse() {
    let mut s: Settings = serde_json::from_str(OLD_SETTINGS).unwrap();
    s.tailscale.tags = vec!["agentvm".into()];
    assert!(matches!(s.validate(&limits()), Err(SettingsError::Tailscale(_))));
}

/// Settings saved before Tailscale existed read as "off, with SSH".
#[test]
fn settings_without_tailscale_keep_it_off() {
    let s: Settings = serde_json::from_str(OLD_SETTINGS).unwrap();
    assert_eq!(s.tailscale, agentvm::domain::tailscale::TailscaleSettings::default());
    assert!(s.validate(&limits()).is_ok());
}

const OLD_SETTINGS: &str =
    r#"{"max_vms":4,"cpus":4,"memory_mb":4096,"timeout_s":1800,"model":"default","default_repo":null}"#;

#[test]
fn settings_reject_out_of_range_values() {
    let ok = Settings {
        max_vms: 4,
        cpus: 4,
        memory_mb: 4096,
        timeout_s: 1800,
        model: Model::default_choice(),
        default_repo: None,
        claude_version: Default::default(),
        s3: None,
        auto_snapshots: Default::default(),
        tailscale: Default::default(),
    };
    for bad in [
        Settings { max_vms: 0, ..ok.clone() },
        Settings { cpus: 0, ..ok.clone() },
        Settings { cpus: 19, ..ok.clone() },
        Settings { memory_mb: 512, ..ok.clone() },
        Settings { memory_mb: 65536, ..ok.clone() },
        Settings { timeout_s: 10, ..ok.clone() },
    ] {
        assert!(bad.validate(&limits()).is_err(), "{bad:?}");
    }
}

#[test]
fn recommended_vms_fit_in_ram() {
    assert_eq!(limits().recommended_vms(4096), 14);
    assert_eq!(limits().recommended_vms(2048), 28);
}

#[test]
fn model_maps_to_cli_flag() {
    assert_eq!(Model::default_choice().cli_name(), None);
    for ok in ["opus", "sonnet[1m]", "opusplan", "fable", "claude-opus-5-5", "claude-haiku-5-5"] {
        assert_eq!(Model::parse(ok).unwrap().cli_name(), Some(ok));
    }
    for bad in ["", "Opus", "opus; rm -rf /", "--dangerous", "a b"] {
        assert!(Model::parse(bad).is_err(), "{bad}");
    }
    let m: Model = serde_json::from_str("\"haiku\"").unwrap();
    assert_eq!(m.as_str(), "haiku");
}

#[test]
fn claude_version_accepts_channels_and_semver_only() {
    use agentvm::domain::settings::ClaudeVersion;
    for ok in ["latest", "stable", "2.1.294", "2.2.0-beta.1"] {
        assert_eq!(ClaudeVersion::parse(ok).unwrap().as_str(), ok);
    }
    for bad in ["", "2.1", "v2.1.0", "latest; rm -rf /", "2.1.0 --evil", "../../x"] {
        assert!(ClaudeVersion::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn settings_without_claude_version_default_to_latest() {
    let s: Settings = serde_json::from_str(
        r#"{"max_vms":4,"cpus":4,"memory_mb":4096,"timeout_s":1800,"model":"default","default_repo":null}"#,
    )
    .unwrap();
    assert_eq!(s.claude_version.as_str(), "latest");
}

#[test]
fn per_vm_resources_are_checked_against_the_mac() {
    let host = limits();
    assert!(host.check_vm(4, 4096).is_ok());
    assert!(host.check_vm(18, 8192).is_ok());
    assert!(host.check_vm(0, 4096).is_err());
    assert!(host.check_vm(19, 4096).is_err());
    assert!(host.check_vm(4, 512).is_err());
    assert!(host.check_vm(4, 65536).is_err());
}

#[test]
fn the_disk_must_keep_its_free_space() {
    use agentvm::domain::disk::{DiskFull, check_free};
    assert!(check_free(20 * 1024, 10 * 1024).is_ok());
    let err = check_free(3 * 1024, 10 * 1024).unwrap_err();
    assert_eq!(err, DiskFull { free_mb: 3 * 1024, min_free_mb: 10 * 1024 });
    assert_eq!(
        err.to_string(),
        "only 3 GB free on this Mac's disk (agentvm keeps at least 10 GB free): free some space"
    );
}
