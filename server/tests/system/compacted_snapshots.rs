//! Snapshots compressed into shared chunks: once compacted (the raw clone gone), a snapshot still
//! restores into a VM that boots with its files, and it costs a fraction of its disk.

use std::time::{Duration, Instant};

use serde_json::json;

use crate::helpers::{TERMINAL, get_json, git, post_json, run_in_shell, start_server, temp_repo, wait_for_state};

#[test]
#[ignore = "needs golden and token"]
fn a_compacted_snapshot_restores_into_a_vm_that_boots_with_its_files() {
    let server = start_server();
    let repo = temp_repo();
    let created =
        post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "interactive": true}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    wait_for_state(&server, &id, |s| s == "running", Duration::from_secs(60));
    run_in_shell(&server, &id, "cd /root/work && echo from-chunks > kept.txt && echo written");
    let snap = post_json(&format!("{}/api/tasks/{id}/snapshot", server.base), &json!({"name": "chunks"}));
    let snap_id = snap["id"].as_str().unwrap_or_else(|| panic!("{snap}")).to_owned();
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
    wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(30));

    // Compacted in the background: its clone goes, its real cost shows.
    let t0 = Instant::now();
    let listed = loop {
        let all = get_json(&format!("{}/api/snapshots", server.base));
        let s = all.as_array().unwrap().iter().find(|s| s["id"] == snap_id.as_str()).cloned().expect("listed");
        if s["compacting"] == false {
            break s;
        }
        assert!(t0.elapsed() < Duration::from_secs(180), "never compacted: {s}");
        std::thread::sleep(Duration::from_millis(500));
    };
    let folder = server.home.path().join("snapshots").join(&snap_id);
    assert!(!folder.join("disk.raw").exists(), "the raw clone is still there");
    assert!(folder.join("disk.manifest.json").is_file());
    let mb = listed["size_mb"].as_u64().unwrap();
    assert!(mb < 2048, "a VM's disk (~3.4 GB of data) compressed: {mb} MB");

    let restored = post_json(&format!("{}/api/snapshots/{snap_id}/restore", server.base), &json!({}));
    let new_id = restored["id"].as_str().unwrap_or_else(|| panic!("{restored}")).to_owned();
    let task = wait_for_state(&server, &new_id, |s| s == "running" || TERMINAL.contains(&s), Duration::from_secs(120));
    assert_eq!(task["status"]["state"], "running", "the restored VM did not come up: {task}");
    let saved = post_json(&format!("{}/api/tasks/{new_id}/save", server.base), &json!({}));
    assert!(saved["commits"].as_u64().unwrap_or(0) >= 1, "{saved}");
    assert!(git(repo.path(), &["show", &format!("agent/{new_id}:kept.txt")]).contains("from-chunks"));
    post_json(&format!("{}/api/tasks/{new_id}/stop", server.base), &json!({}));
    wait_for_state(&server, &new_id, |s| TERMINAL.contains(&s), Duration::from_secs(30));
}
