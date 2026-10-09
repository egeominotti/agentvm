//! A new VM boots straight into the kernel kept beside the golden image.

use std::time::{Duration, Instant};

use serde_json::json;

use crate::helpers::{TERMINAL, get_json, post_json, start_server, temp_repo, wait_for_state};

#[test]
#[ignore = "needs golden, built with its kernel beside it"]
fn a_new_vm_boots_straight_into_the_kernel_of_its_image() {
    let server = start_server();
    let repo = temp_repo();
    let created =
        post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "interactive": true}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let t0 = Instant::now();
    let task = loop {
        let task = get_json(&format!("{}/api/tasks/{id}", server.base));
        if task["ready"] == true {
            break task;
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "never ready: {task}");
        std::thread::sleep(Duration::from_millis(50));
    };
    let boot = task["boot_log"].to_string();
    assert!(boot.contains("booting straight into its kernel"), "{boot}");
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
    wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(30));
}
