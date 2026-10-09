//! What the settings read and offer: the settings file, the Mac's limits, Claude Code releases.

#[test]
fn settings_file_roundtrips_and_is_absent_at_first() {
    use agentvm::adapters::settings_file;
    use agentvm::domain::settings::{Model, Settings};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    assert_eq!(settings_file::load(&path), settings_file::Saved::Missing);
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
        tailscale: Default::default(),
    };
    settings_file::save(&path, &s).unwrap();
    assert_eq!(settings_file::load(&path), settings_file::Saved::Json(serde_json::to_value(&s).unwrap()));
}

/// A settings file that is not JSON is reported as such and can be kept aside as it was.
#[test]
fn a_corrupt_settings_file_is_kept_aside() {
    use agentvm::adapters::settings_file;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    std::fs::write(&path, "{\"max_vms\": 4,").unwrap();
    assert_eq!(settings_file::load(&path), settings_file::Saved::Corrupt);
    let copy = settings_file::keep_copy(&path).unwrap();
    assert_eq!(copy, tmp.path().join("settings.json.bad"));
    assert_eq!(std::fs::read_to_string(copy).unwrap(), "{\"max_vms\": 4,");
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

/// The server keeps a socket per terminal, port forward and VM connection: launchd's default of
/// 256 open files is far too few for twenty VMs.
#[test]
fn the_open_files_limit_is_raised() {
    let limit = agentvm::adapters::host::raise_open_files_limit().unwrap();
    assert!(limit >= 4096, "{limit}");
}

/// Facts about this Mac are read from the kernel (as `sysctl` prints them), not by running a
/// process for each: the scheduler asks for free memory on every launch, telemetry often.
#[test]
fn host_facts_come_straight_from_the_kernel() {
    let sysctl = |name: &str| -> u64 {
        let out = std::process::Command::new("sysctl").args(["-n", name]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
    };
    let host = agentvm::adapters::host::host_limits();
    assert_eq!(u64::from(host.cpus), sysctl("hw.ncpu"));
    assert_eq!(host.ram_mb, sysctl("hw.memsize") >> 20);
    let t = std::time::Instant::now();
    for _ in 0..100 {
        assert!(agentvm::adapters::host::memory_free_mb().is_some());
    }
    assert!(t.elapsed() < std::time::Duration::from_millis(50), "100 reads took {:?}", t.elapsed());
}

/// Free disk space, checked before every launch and every file a guest writes: what `df` says,
/// read with the same system call instead of a `df` process each time.
#[test]
fn free_disk_space_is_what_df_reports() {
    let tmp = tempfile::tempdir().unwrap();
    let out = std::process::Command::new("df").args(["-k", "-P"]).arg(tmp.path()).output().unwrap();
    let df_mb: u64 = String::from_utf8_lossy(&out.stdout)
        .lines()
        .nth(1)
        .unwrap()
        .split_whitespace()
        .nth(3)
        .unwrap()
        .parse::<u64>()
        .unwrap()
        >> 10;
    let ours = agentvm::adapters::host::free_mb(tmp.path()).expect("free space");
    // Other programs write meanwhile: the same figure within 1 GB.
    assert!(ours.abs_diff(df_mb) < 1024, "{ours} MB vs df's {df_mb} MB");
    assert_eq!(agentvm::adapters::host::free_mb(&tmp.path().join("missing")), None);
    let t = std::time::Instant::now();
    for _ in 0..100 {
        agentvm::adapters::host::free_mb(tmp.path()).unwrap();
    }
    assert!(t.elapsed() < std::time::Duration::from_millis(50), "100 reads took {:?}", t.elapsed());
}

/// What the scheduler asks before starting a VM: this Mac's free memory, as macOS judges it.
#[test]
fn the_mac_says_how_much_memory_is_free() {
    let free = agentvm::adapters::host::memory_free_mb().expect("kern.memorystatus_level");
    let total = agentvm::adapters::host::host_limits().ram_mb;
    assert!(free > 0 && free <= total, "{free} of {total} MB");
}
