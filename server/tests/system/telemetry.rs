//! A real VM's telemetry: what it does shows up, measured by its own collector.

use std::time::{Duration, Instant};

use serde_json::json;

use crate::helpers::{get_json, post_json, run_in_shell, start_server, temp_repo, wait_for_state};

/// Writing 400 MB inside the VM shows as disk writes; the processes using the most memory and
/// how much memory the VM may keep are reported, and the numbers are fresh.
#[test]
#[ignore = "needs golden and token"]
fn disk_writes_and_memory_show_in_the_telemetry() {
    let server = start_server();
    let repo = temp_repo();
    let created = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "interactive": true}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    wait_for_state(&server, &id, |s| s == "running", Duration::from_secs(60));
    run_in_shell(&server, &id, "dd if=/dev/zero of=/root/big bs=1M count=400 oflag=direct status=none; echo wrote");
    let t0 = Instant::now();
    loop {
        let series = get_json(&format!("{}/api/tasks/{id}/telemetry?range=5m", server.base));
        let wrote = series["points"].as_array().unwrap().iter().any(|p| p["disk_write_bps"].as_u64().unwrap_or(0) > 10 << 20);
        if wrote {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(20), "no disk writes seen: {series}");
        std::thread::sleep(Duration::from_secs(1));
    }
    let task = get_json(&format!("{}/api/tasks/{id}", server.base));
    assert!(!task["metrics"]["top_mem"].as_array().unwrap().is_empty(), "{task}");
    assert!(task["memory_limit_mb"].as_u64().unwrap_or(0) > 0, "{task}");
    assert!(task["metrics_age_s"].as_f64().unwrap_or(99.0) < 5.0, "{task}");
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
}
