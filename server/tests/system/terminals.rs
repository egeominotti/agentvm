//! Interactive terminals: saves to the branch, the resources asked for, files dropped on them.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::helpers::{
    TERMINAL, assert_no_token, curl, get_json, git, post_json, run_in_shell, start_server, temp_repo, wait_for_state,
};

#[test]
#[ignore = "requires golden, token and Claude"]
fn interactive_terminal_saves_to_the_branch_and_closes() {
    let server = start_server();
    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "interactive": true,
                "prompt": "Create the file term.txt containing hello and commit it."}),
    );
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let t0 = Instant::now();
    loop {
        let task = get_json(&format!("{}/api/tasks/{id}", server.base));
        assert_ne!(task["status"]["state"], "failed", "{task}");
        if task["activity"] == "waiting" && task["status"]["state"] == "running" {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(240), "Claude did not finish its turn: {task}");
        std::thread::sleep(Duration::from_millis(500));
    }
    // Cost and tokens arrive through Claude Code's status line.
    let t1 = Instant::now();
    let usage = loop {
        let task = get_json(&format!("{}/api/tasks/{id}", server.base));
        if task["usage"]["output_tokens"].as_u64().unwrap_or(0) > 0 {
            break task["usage"].clone();
        }
        assert!(t1.elapsed() < Duration::from_secs(30), "no usage: {task}");
        std::thread::sleep(Duration::from_millis(500));
    };
    assert!(usage["cost_usd"].as_f64().unwrap() > 0.0 && usage["input_tokens"].as_u64().unwrap() > 0, "{usage}");
    // Nothing in Claude's screen waits for a human: no permission or mode prompts.
    let screen = run_in_shell(&server, &id, "tmux capture-pane -p -t claude -S -300");
    assert!(screen.contains("Claude Code"), "did not read Claude's screen:\n{screen}");
    for prompt in ["Make auto mode your default", "Do you trust", "Yes, I accept"] {
        assert!(!screen.contains(prompt), "Claude is blocked on a prompt ({prompt}):\n{screen}");
    }
    let saved = post_json(&format!("{}/api/tasks/{id}/save", server.base), &json!({}));
    assert!(saved["commits"].as_u64().unwrap_or(0) >= 1, "{saved}");
    let content = git(repo.path(), &["show", &format!("agent/{id}:term.txt")]);
    assert!(content.to_lowercase().contains("hello"));

    post_json(&format!("{}/api/tasks/{id}/close", server.base), &json!({}));
    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(60));
    assert_eq!(task["status"]["state"], "done", "{task}");
    assert_no_token(&server, repo.path(), Some(&format!("agent/{id}")));
}

#[test]
#[ignore = "needs golden and token"]
fn launch_resources_reach_the_vm() {
    let server = start_server();
    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "interactive": true, "cpus": 2, "memory_mb": 2048}),
    );
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let t0 = Instant::now();
    let metrics = loop {
        let task = get_json(&format!("{}/api/tasks/{id}", server.base));
        if task["metrics"].is_object() {
            break task["metrics"].clone();
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "no telemetry: {task}");
        std::thread::sleep(Duration::from_millis(500));
    };
    assert_eq!(metrics["cpus"], 2, "{metrics}");
    let mem = metrics["mem_total_mb"].as_u64().unwrap();
    assert!((1700..=2048).contains(&mem), "{metrics}");
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
}

/// Files dropped on a terminal land inside the VM, where Claude can open them.
#[test]
#[ignore = "needs golden and token"]
fn files_dropped_on_a_terminal_reach_the_vm() {
    let server = start_server();
    let repo = temp_repo();
    let created =
        post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "interactive": true}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    wait_for_state(&server, &id, |s| s == "running", Duration::from_secs(60));
    let out = curl(&[
        "-s",
        "-X",
        "POST",
        "-H",
        "x-file-name: my%20notes.txt",
        "--data-binary",
        "dropped-ok",
        &format!("{}/api/tasks/{id}/upload", server.base),
    ])
    .unwrap();
    let path =
        serde_json::from_str::<Value>(&out).unwrap()["path"].as_str().unwrap_or_else(|| panic!("{out}")).to_owned();
    assert_eq!(path, "/mnt/job/uploads/my notes.txt");
    let t0 = Instant::now();
    loop {
        if run_in_shell(&server, &id, &format!("cat '{path}'")).contains("dropped-ok") {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "the file never showed up in the VM");
    }
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
    wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(30));
}

/// The VM's terminal and telemetry services come back by themselves when they die (a crash,
/// the guest's out-of-memory killer): the terminals and the dashboard's numbers keep working.
#[test]
#[ignore = "needs golden and token"]
fn guest_services_come_back_after_dying() {
    let server = start_server();
    let repo = temp_repo();
    let created =
        post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "interactive": true}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    wait_for_state(&server, &id, |s| s == "running", Duration::from_secs(60));
    run_in_shell(&server, &id, "echo up");
    // Our own connection goes down with the terminal server: whatever it prints is lost.
    run_in_shell(&server, &id, "pkill -9 -f agentvm-metrics; pkill -9 -f agentvm-pty");
    assert!(run_in_shell(&server, &id, "echo back-$((2+3))").contains("back-5"), "the terminals never came back");
    let uptime = || get_json(&format!("{}/api/tasks/{id}", server.base))["metrics"]["uptime_s"].as_u64().unwrap_or(0);
    let before = uptime();
    std::thread::sleep(Duration::from_secs(5));
    assert!(uptime() > before, "the VM's numbers stopped at {before}");
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
}
