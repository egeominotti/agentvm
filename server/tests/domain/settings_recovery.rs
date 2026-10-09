//! Saved settings that no longer fit (another Mac, an older version, a hand edit) lose only the
//! fields that are wrong, never everything.

use agentvm::domain::settings::{HostLimits, Model, Settings};
use agentvm::domain::settings_recovery::recover;
use serde_json::json;

fn defaults() -> Settings {
    Settings {
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
    }
}

#[test]
fn a_field_that_no_longer_fits_this_mac_is_reset_alone() {
    let mut saved = serde_json::to_value(defaults()).unwrap();
    saved["cpus"] = json!(16);
    saved["max_vms"] = json!(7);
    saved["default_repo"] = json!("/Users/me/code/app");
    let (s, reset) = recover(&saved, &defaults(), &HostLimits { cpus: 8, ram_mb: 65536 });
    assert_eq!(reset, vec!["cpus".to_owned()]);
    assert_eq!((s.cpus, s.max_vms, s.default_repo.as_deref()), (4, 7, Some("/Users/me/code/app")));
}

#[test]
fn wrong_types_and_missing_fields_keep_everything_else() {
    let saved = json!({ "max_vms": 9, "timeout_s": "half an hour", "model": "sonnet", "extra": true });
    let (s, reset) = recover(&saved, &defaults(), &HostLimits { cpus: 18, ram_mb: 65536 });
    assert_eq!(reset, vec!["timeout_s".to_owned()]);
    assert_eq!((s.max_vms, s.timeout_s), (9, 1800));
    assert_eq!(s.model.as_str(), "sonnet");
    assert!(s.validate(&HostLimits { cpus: 18, ram_mb: 65536 }).is_ok());
}

#[test]
fn anything_but_an_object_gives_the_defaults() {
    let (s, reset) = recover(&json!([1, 2]), &defaults(), &HostLimits { cpus: 18, ram_mb: 65536 });
    assert_eq!(s, defaults());
    assert_eq!(reset, vec!["(the whole file)".to_owned()]);
}

/// A small Mac cannot give VMs the default 4 GB: the defaults are fitted to it first, so the
/// valid saved values around that default are never reset (they were, at every start).
#[test]
fn a_default_that_does_not_fit_this_mac_resets_nothing_saved() {
    let small = HostLimits { cpus: 8, ram_mb: 8 * 1024 };
    let mut saved = serde_json::to_value(defaults()).unwrap();
    saved["memory_mb"] = json!(1024);
    saved["max_vms"] = json!(3);
    saved["default_repo"] = json!("/Users/me/app");
    let (s, reset) = recover(&saved, &defaults(), &small);
    assert!(reset.is_empty(), "{reset:?}");
    assert_eq!((s.memory_mb, s.max_vms, s.default_repo.as_deref()), (1024, 3, Some("/Users/me/app")));
}

/// A time limit from the environment out of range must not wipe a good file, S3 included.
#[test]
fn a_valid_file_is_kept_whole_whatever_the_defaults() {
    let host = HostLimits { cpus: 18, ram_mb: 65536 };
    let mut bad_defaults = defaults();
    bad_defaults.timeout_s = 5;
    let mut saved = serde_json::to_value(defaults()).unwrap();
    saved["max_vms"] = json!(9);
    saved["s3"] = json!({"endpoint":"https://s3.example.com","region":"auto","bucket":"agentvm-backups","prefix":"agentvm","access_key":"k","path_style":true});
    let (s, reset) = recover(&saved, &bad_defaults, &host);
    assert!(reset.is_empty(), "{reset:?}");
    assert_eq!(s.max_vms, 9);
    assert!(s.s3.is_some());
}

/// Defaults that do not fit are fitted, never used as they are.
#[test]
fn defaults_are_fitted_to_the_mac() {
    let small = HostLimits { cpus: 4, ram_mb: 8 * 1024 };
    let mut d = defaults();
    d.cpus = 18;
    d.timeout_s = 5;
    let fitted = d.fitted(&small);
    assert!(fitted.validate(&small).is_ok(), "{fitted:?}");
    assert_eq!((fitted.cpus, fitted.memory_mb, fitted.timeout_s), (4, 1024, 60));
}
