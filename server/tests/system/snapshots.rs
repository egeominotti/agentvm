//! Snapshots taken by hand or on a schedule, and the VMs restored from them.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::helpers::{
    TERMINAL, curl, delete, get_json, git, post_json, put_json, start_server, temp_repo, wait_for_state,
};

#[test]
#[ignore = "needs golden, token and Claude"]
fn a_snapshot_restores_files_into_a_new_vm() {
    let server = start_server();
    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "interactive": true,
                "prompt": "Create the file snap.txt containing snapshot-ok. Do not commit it."}),
    );
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let t0 = Instant::now();
    while get_json(&format!("{}/api/tasks/{id}", server.base))["activity"] != "waiting" {
        assert!(t0.elapsed() < Duration::from_secs(240), "Claude did not finish its turn");
        std::thread::sleep(Duration::from_millis(500));
    }
    let snap = post_json(&format!("{}/api/tasks/{id}/snapshot", server.base), &json!({"name": "with snap.txt"}));
    let snap_id = snap["id"].as_str().unwrap_or_else(|| panic!("{snap}")).to_owned();
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
    wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(30));

    let listed = get_json(&format!("{}/api/snapshots", server.base));
    assert!(listed.as_array().unwrap().iter().any(|s| s["id"] == snap_id.as_str()), "{listed}");
    let restored = post_json(&format!("{}/api/snapshots/{snap_id}/restore", server.base), &json!({}));
    let new_id = restored["id"].as_str().unwrap_or_else(|| panic!("{restored}")).to_owned();
    wait_for_state(&server, &new_id, |s| s == "running", Duration::from_secs(60));
    let saved = post_json(&format!("{}/api/tasks/{new_id}/save", server.base), &json!({}));
    assert!(saved["commits"].as_u64().unwrap_or(0) >= 1, "{saved}");
    let content = git(repo.path(), &["show", &format!("agent/{new_id}:snap.txt")]);
    assert!(content.contains("snapshot-ok"), "{content}");
    post_json(&format!("{}/api/tasks/{new_id}/stop", server.base), &json!({}));
}

#[test]
#[ignore = "needs golden and token"]
fn restore_keeps_resources_and_finished_tasks_can_be_removed() {
    let server = start_server();
    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "interactive": true, "cpus": 2, "memory_mb": 2048}),
    );
    let id = created["id"].as_str().unwrap().to_owned();
    wait_for_state(&server, &id, |s| s == "running", Duration::from_secs(60));
    std::thread::sleep(Duration::from_secs(2));
    let snap = post_json(&format!("{}/api/tasks/{id}/snapshot", server.base), &json!({}));
    let sid = snap["id"].as_str().unwrap_or_else(|| panic!("{snap}")).to_owned();
    assert!(!snap["name"].as_str().unwrap().starts_with('/'), "default name is not a path: {snap}");
    assert_eq!(snap["cpus"], 2, "{snap}");
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
    wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(30));

    let restored = post_json(&format!("{}/api/snapshots/{sid}/restore", server.base), &json!({}));
    let rid = restored["id"].as_str().unwrap().to_owned();
    let task = get_json(&format!("{}/api/tasks/{rid}", server.base));
    assert_eq!(task["cpus"], 2, "{task}");
    assert_eq!(task["memory_mb"], 2048, "{task}");
    assert!(task["label"].as_str().unwrap_or_default().starts_with("Restored: "), "{task}");
    post_json(&format!("{}/api/tasks/{rid}/stop", server.base), &json!({}));
    wait_for_state(&server, &rid, |s| TERMINAL.contains(&s), Duration::from_secs(30));

    assert_eq!(delete(&format!("{}/api/tasks/{id}", server.base)), 204);
    assert!(curl(&["-sf", &format!("{}/api/tasks/{id}", server.base)]).is_none(), "removed from the list");
    assert!(!server.home.path().join("jobs").join(&id).exists(), "job folder deleted");
}

/// Snapshots on a schedule, pruned to the newest `keep`, plus one just before closing.
#[test]
#[ignore = "needs golden and token"]
fn running_terminals_are_snapshotted_on_a_schedule_and_before_closing() {
    let server = start_server();
    let mut s = get_json(&format!("{}/api/settings", server.base))["settings"].clone();
    s["auto_snapshots"] = json!({"every_min": 1, "keep": 1, "before_close": true});
    let saved = put_json(&format!("{}/api/settings", server.base), &s);
    assert_eq!(saved["settings"]["auto_snapshots"]["every_min"], 1, "{saved}");

    let repo = temp_repo();
    let created =
        post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "interactive": true}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let autos = || -> Vec<Value> {
        get_json(&format!("{}/api/snapshots", server.base))
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["source_task"] == id.as_str() && s["auto"] == true)
            .cloned()
            .collect()
    };
    let t0 = Instant::now();
    while autos().is_empty() {
        assert!(t0.elapsed() < Duration::from_secs(150), "no automatic snapshot");
        std::thread::sleep(Duration::from_secs(2));
    }
    post_json(&format!("{}/api/tasks/{id}/close", server.base), &json!({}));
    wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(60));
    let left = autos();
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(left[0]["name"].as_str().unwrap().contains("before close"), "{left:?}");
}
