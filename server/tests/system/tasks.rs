//! Automatic tasks: a prompt in, a branch out, and every way a task can end.

use std::process::Command;
use std::time::Duration;

use serde_json::json;

use crate::helpers::{
    TERMINAL, assert_no_token, curl, get_json, git, post_json, put_json, run_task, start_server, temp_repo,
    wait_for_state,
};

#[test]
#[ignore = "requires golden, token and Claude"]
fn task_produces_a_branch_in_the_local_repo() {
    let server = start_server();
    let status = get_json(&format!("{}/api/status", server.base));
    assert_eq!(status["golden"], true);
    assert_eq!(status["token"], true);

    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "prompt": "Create the file hello.txt containing hello and commit it."}),
    );
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();

    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(300));
    assert_eq!(task["status"]["state"], "done", "{task}");
    let branch = format!("agent/{id}");
    assert_eq!(task["branch"], branch.as_str());
    assert!(git(repo.path(), &["show", &format!("{branch}:hello.txt")]).to_lowercase().contains("hello"));
    assert!(curl(&["-sf", &format!("{}/api/tasks/{id}/diff", server.base)]).unwrap().contains("hello.txt"));

    // curl exits with an error when --max-time expires, but the output received is valid.
    let out = Command::new("curl")
        .args(["-sN", "--max-time", "2", &format!("{}/api/tasks/{id}/events", server.base)])
        .output()
        .unwrap();
    let sse = String::from_utf8_lossy(&out.stdout);
    assert!(sse.contains("event: state") && sse.contains("event: agent") && sse.contains("tool_use"), "{sse}");
    assert_no_token(&server, repo.path(), Some(&branch));
}

#[test]
#[ignore = "requires golden, token and Claude"]
fn running_task_can_be_stopped() {
    let server = start_server();
    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "prompt": "Run the command `sleep 120` with Bash and then create x.txt."}),
    );
    let id = created["id"].as_str().unwrap().to_owned();
    wait_for_state(&server, &id, |s| s == "running", Duration::from_secs(30));
    let stopped = post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
    assert_eq!(stopped["status"]["state"], "running", "{stopped}");
    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(30));
    assert_eq!(task["status"]["state"], "stopped");
    assert!(git(repo.path(), &["branch", "--list", &format!("agent/{id}")]).trim().is_empty());
}

#[test]
#[ignore = "requires golden, token and Claude"]
fn invalid_requests_are_rejected_with_a_message() {
    let server = start_server();
    let not_repo = tempfile::tempdir().unwrap();
    let r = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": not_repo.path(), "prompt": "x"}));
    assert!(r["error"].as_str().unwrap().contains("is not a git repository"), "{r}");
    let repo = temp_repo();
    let r = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "prompt": "  "}));
    assert!(r["error"].as_str().unwrap().contains("empty"), "{r}");
    assert!(curl(&["-sf", &format!("{}/api/tasks/nope", server.base)]).is_none());
}

#[test]
#[ignore = "requires golden, token and Claude"]
fn prompt_starting_with_a_dash_reaches_claude() {
    let server = start_server();
    let repo = temp_repo();
    let (id, task) = run_task(&server, repo.path(), "- create the file dash.txt containing ok\n- commit it");
    assert_eq!(task["status"]["state"], "done", "{task}");
    git(repo.path(), &["show", &format!("agent/{id}:dash.txt")]);
}

#[test]
#[ignore = "requires golden, token and Claude"]
fn commits_on_another_branch_are_not_lost() {
    let server = start_server();
    let repo = temp_repo();
    let (id, task) = run_task(
        &server,
        repo.path(),
        "Run `git checkout -b feature/x`, then create x.txt containing x and commit it on that branch.",
    );
    assert_eq!(task["status"]["state"], "done", "{task}");
    git(repo.path(), &["show", &format!("agent/{id}:x.txt")]);
}

#[test]
#[ignore = "requires golden and token"]
fn stop_right_after_submit_never_leaves_the_task_hanging() {
    let server = start_server();
    let repo = temp_repo();
    let created =
        post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "prompt": "create a.txt"}));
    let id = created["id"].as_str().unwrap().to_owned();
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(10));
    assert_eq!(task["status"]["state"], "stopped", "{task}");
    let console = server.home.path().join(format!("jobs/{id}/console.log"));
    assert!(!console.exists() || std::fs::metadata(&console).unwrap().len() == 0, "the VM started anyway");
}

/// An automatic task that runs out of time keeps the commits it made.
#[test]
#[ignore = "requires golden, token and Claude"]
fn a_task_that_runs_out_of_time_keeps_its_commits() {
    let server = start_server();
    let mut s = get_json(&format!("{}/api/settings", server.base))["settings"].clone();
    s["timeout_s"] = 60.into();
    put_json(&format!("{}/api/settings", server.base), &s);
    let repo = temp_repo();
    let (id, task) = run_task(
        &server,
        repo.path(),
        // Not `sleep 600`: Claude Code now blocks a standalone sleep, and the task would end early.
        "Create the file early.txt containing early and commit it. Then run the shell command \
         `tail -f /dev/null` in the foreground (it waits forever, that is intended) and wait for it to finish.",
    );
    assert_eq!(task["status"]["state"], "failed", "{task}");
    assert!(task["status"]["reason"].as_str().unwrap_or_default().contains("timeout"), "{task}");
    assert!(git(repo.path(), &["show", &format!("agent/{id}:early.txt")]).contains("early"), "the commit was lost");
}
