//! What the settings read and offer: the settings file, the Mac's limits, Claude Code releases.

#[test]
fn settings_file_roundtrips_and_is_absent_at_first() {
    use agentvm::adapters::settings_file;
    use agentvm::domain::settings::{Model, Settings};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    assert!(settings_file::load(&path).is_none());
    let s = Settings {
        max_vms: 6,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 900,
        model: Model::parse("opus").unwrap(),
        default_repo: Some("~/x".into()),
        claude_version: agentvm::domain::settings::ClaudeVersion::parse("2.1.290").unwrap(),
        s3: None,
        auto_snapshots: Default::default(),
    };
    settings_file::save(&path, &s).unwrap();
    assert_eq!(settings_file::load(&path).unwrap(), s);
}

#[test]
fn host_info_reports_this_mac() {
    let host = agentvm::adapters::host::host_limits();
    assert!(host.cpus >= 1 && host.ram_mb >= 1024, "{host:?}");
}

#[test]
#[ignore = "needs the network"]
fn claude_releases_lists_real_versions() {
    let r = agentvm::adapters::releases::fetch().unwrap();
    let semver = |v: &str| {
        v.split('.').count() == 3 && v.split('.').all(|p| p.chars().next().is_some_and(|c| c.is_ascii_digit()))
    };
    assert!(semver(&r.latest) && semver(&r.stable), "{r:?}");
    assert!(r.versions.len() >= 10, "{r:?}");
    assert!(r.versions.contains(&r.latest), "{r:?}");
    assert!(semver(&r.versions[0]));
}
