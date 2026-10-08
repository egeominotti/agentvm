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
