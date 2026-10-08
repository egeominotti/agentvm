//! The server process: one per home, and VMs that outlive it.

use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::helpers::{
    TERMINAL, curl, launch_at, post_json, spawn_server, start_server, start_server_with, temp_repo, test_home,
    wait_for_state,
};

#[test]
#[ignore = "requires golden and bin"]
fn second_server_on_the_same_home_refuses_to_start() {
    let server = start_server();
    let mut second = spawn_server(server.home.path());
    let t0 = Instant::now();
    let status = loop {
        if let Some(st) = second.try_wait().unwrap() {
            break st;
        }
        if t0.elapsed() > Duration::from_secs(5) {
            let _ = second.kill();
            panic!("the second server started on the same home");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(!status.success());
    let mut err = String::new();
    std::io::Read::read_to_string(second.stderr.as_mut().unwrap(), &mut err).unwrap();
    assert!(err.contains("already running"), "{err}");
    assert!(curl(&["-sf", &format!("{}/api/status", server.base)]).is_some());
}

#[test]
#[ignore = "needs golden and token"]
fn vms_survive_a_server_restart() {
    let home = test_home();
    let home_path = home.path().to_path_buf();
    let mut server = start_server_with(home, &[]);
    let repo = temp_repo();
    let created =
        post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "interactive": true}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    wait_for_state(&server, &id, |s| s == "running", Duration::from_secs(60));
    let pid: i32 =
        std::fs::read_to_string(home_path.join(format!("jobs/{id}/vm.pid"))).unwrap().trim().parse().unwrap();

    // Kill the server hard: no cleanup code runs.
    server.child.kill().unwrap();
    server.child.wait().unwrap();
    std::thread::sleep(Duration::from_secs(1));
    let alive = Command::new("kill").args(["-0", &pid.to_string()]).status().unwrap().success();
    assert!(alive, "the VM died with the server");

    // A new server on the same home finds the VM and drives it.
    // The first `server` still owns (and deletes) the home: this one runs on it without owning it.
    let again = launch_at(&home_path, tempfile::tempdir().unwrap(), &[]);
    let task = wait_for_state(&again, &id, |s| s == "running", Duration::from_secs(10));
    assert_eq!(task["interactive"], true);
    std::fs::write(home_path.join("jobs").join(&id).join("share/marker.txt"), "x").ok();
    let saved = post_json(&format!("{}/api/tasks/{id}/save", again.base), &json!({}));
    assert!(saved["commits"].is_number(), "{saved}");
    post_json(&format!("{}/api/tasks/{id}/close", again.base), &json!({}));
    let done = wait_for_state(&again, &id, |s| TERMINAL.contains(&s), Duration::from_secs(60));
    assert!(matches!(done["status"]["state"].as_str(), Some("done" | "no_changes")), "{done}");
    let gone = !Command::new("kill").args(["-0", &pid.to_string()]).status().unwrap().success();
    assert!(gone, "the VM is still running after close");
}
