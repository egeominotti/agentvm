//! Snapshots backed up to S3, restored from it, exported and imported.

use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};

use crate::helpers::{delete, get_json, post_json, start_server_with, temp_repo, test_home, wait_for_state};

#[test]
#[ignore = "needs golden, token and the dev S3 server (scripts/dev-s3.sh up)"]
fn snapshots_go_to_s3_and_come_back() {
    let home = test_home();
    let prefix = format!("system-test-{}", std::process::id());
    let settings = json!({"max_vms": 4, "cpus": 2, "memory_mb": 2048, "timeout_s": 1800, "model": "default",
        "default_repo": null, "claude_version": "latest",
        "s3": {"endpoint": "http://127.0.0.1:9100", "region": "us-east-1", "bucket": "agentvm-backups",
               "prefix": prefix, "access_key": "agentvm", "path_style": true}});
    std::fs::write(home.path().join("settings.json"), settings.to_string()).unwrap();
    let server = start_server_with(home, &[("AGENTVM_S3_SECRET", "agentvm-local-secret")]);
    let base = server.base.clone();
    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "-X", "POST", &format!("{base}/api/settings/s3/test")])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "204", "S3 connection test");

    let repo = temp_repo();
    let created = post_json(&format!("{base}/api/tasks"), &json!({"repo_path": repo.path(), "interactive": true}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    wait_for_state(&server, &id, |s| s == "running", Duration::from_secs(60));
    std::thread::sleep(Duration::from_secs(2));
    let snap = post_json(&format!("{base}/api/tasks/{id}/snapshot"), &json!({"name": "to s3"}));
    let sid = snap["id"].as_str().unwrap_or_else(|| panic!("{snap}")).to_owned();
    post_json(&format!("{base}/api/tasks/{id}/stop"), &json!({}));

    let backed = post_json(&format!("{base}/api/snapshots/{sid}/backup"), &json!({}));
    assert!(backed["archive_mb"].as_u64().unwrap_or(0) > 0, "{backed}");
    assert_eq!(delete(&format!("{base}/api/snapshots/{sid}")), 204);
    let remote = get_json(&format!("{base}/api/backups"));
    let entry = remote
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["snapshot"]["id"] == sid.as_str())
        .unwrap_or_else(|| panic!("{remote}"))
        .clone();
    assert_eq!(entry["local"], false);

    let restored = post_json(&format!("{base}/api/backups/{sid}/restore"), &json!({}));
    assert_eq!(restored["id"], sid.as_str(), "{restored}");
    assert!(get_json(&format!("{base}/api/snapshots")).as_array().unwrap().iter().any(|s| s["id"] == sid.as_str()));

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("snap.tar.zst");
    let out = Command::new("curl")
        .args(["-sf", "-o"])
        .arg(&file)
        .arg(format!("{base}/api/snapshots/{sid}/export"))
        .output()
        .unwrap();
    assert!(out.status.success() && std::fs::metadata(&file).unwrap().len() > 1 << 20, "export");
    let imported = Command::new("curl")
        .args(["-s", "-X", "POST", "--data-binary"])
        .arg(format!("@{}", file.display()))
        .arg(format!("{base}/api/snapshots/import"))
        .output()
        .unwrap();
    let imported: Value = serde_json::from_slice(&imported.stdout).unwrap();
    assert_ne!(imported["id"], sid.as_str(), "{imported}");
    assert_eq!(imported["name"], "to s3");

    assert_eq!(delete(&format!("{base}/api/backups/{sid}")), 204);
    assert!(
        !get_json(&format!("{base}/api/backups"))
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["snapshot"]["id"] == sid.as_str())
    );
}
