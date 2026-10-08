//! What Claude gets inside its VM: root, a browser, a pinned version, and never the token.

use std::process::Command;
use std::time::Duration;

use serde_json::json;

use crate::helpers::{
    TERMINAL, assert_no_token, get_json, git, post_json, run_task, start_server, temp_repo, wait_for_state,
};

/// Claude's commands do not see the token. Only the length of the variable is written: asked to
/// commit the token itself, Claude rightly refuses some of the time, and the test would depend
/// on that choice.
#[test]
#[ignore = "requires golden, token and Claude"]
fn agent_commands_cannot_see_the_token() {
    let server = start_server();
    let repo = temp_repo();
    let (id, task) = run_task(
        &server,
        repo.path(),
        "Run `printenv CLAUDE_CODE_OAUTH_TOKEN | wc -c | tr -d ' ' > token-length.txt` with Bash, \
         then commit token-length.txt.",
    );
    assert_eq!(task["status"]["state"], "done", "{task}");
    let length = git(repo.path(), &["show", &format!("agent/{id}:token-length.txt")]);
    assert_eq!(length.trim(), "0", "the token is visible to the agent's commands");
    assert_no_token(&server, repo.path(), Some(&format!("agent/{id}")));
}

#[test]
#[ignore = "requires golden, token and Claude"]
fn claude_runs_as_root_with_full_permissions() {
    let server = start_server();
    let repo = temp_repo();
    let (id, task) = run_task(
        &server,
        repo.path(),
        "Without using sudo: run `id -u > perm.txt`, then `apt-get install -y -q sl >/dev/null 2>&1; dpkg-query -W -f='${Status}' sl >> perm.txt`. Finally commit perm.txt.",
    );
    assert_eq!(task["status"]["state"], "done", "{task}");
    let perm = git(repo.path(), &["show", &format!("agent/{id}:perm.txt")]);
    let mut lines = perm.lines();
    assert_eq!(lines.next(), Some("0"), "{perm}");
    assert!(perm.contains("install ok installed"), "{perm}");
}

#[test]
#[ignore = "needs golden, token, Claude and the network"]
fn a_launch_can_pin_another_claude_code_version() {
    let server = start_server();
    let releases = get_json(&format!("{}/api/claude/versions", server.base));
    // An older release than the image's (the image follows "latest").
    let latest = releases["latest"].as_str().unwrap().to_owned();
    let other = releases["versions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .find(|v| *v != latest)
        .unwrap()
        .to_owned();
    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "claude_version": other, "prompt": "Reply with the single word ok."}),
    );
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(300));
    assert_ne!(task["status"]["state"], "failed", "{task}");
    let out = Command::new("curl")
        .args(["-sN", "--max-time", "2", &format!("{}/api/tasks/{id}/events", server.base)])
        .output()
        .unwrap();
    let sse = String::from_utf8_lossy(&out.stdout);
    assert!(sse.contains(&format!("\"claude_code_version\":\"{other}\"")), "wanted {other}: {sse}");
}

#[test]
#[ignore = "requires golden, token and Claude"]
fn claude_has_a_browser_out_of_the_box() {
    let server = start_server();
    let repo = temp_repo();
    // The title only exists once JavaScript runs: reading the file is not enough.
    std::fs::write(
        repo.path().join("index.html"),
        "<title>loading</title><script>document.title = 'agentvm-' + (6 * 7)</script>\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "page"]);
    let (id, task) = run_task(
        &server,
        repo.path(),
        "Serve this repository with `python3 -m http.server 8123` in the background, open \
         http://localhost:8123/ with your browser tool, and write the page title the browser shows \
         (nothing else) into title.txt. Commit title.txt.",
    );
    assert_eq!(task["status"]["state"], "done", "{task}");
    let out = Command::new("curl")
        .args(["-sN", "--max-time", "2", &format!("{}/api/tasks/{id}/events", server.base)])
        .output()
        .unwrap();
    let sse = String::from_utf8_lossy(&out.stdout);
    assert!(sse.contains("mcp__playwright__browser_navigate"), "Claude did not use the browser: {sse}");
    assert_eq!(git(repo.path(), &["show", &format!("agent/{id}:title.txt")]).trim(), "agentvm-42");
}

/// An interactive VM's conversation reaches the host as it happens and stays once the VM is
/// closed: the prompt, Claude's tool calls, and its usage over time.
#[test]
#[ignore = "requires golden, token and Claude"]
fn claudes_conversation_is_kept_after_the_vm_closes() {
    use std::time::{Duration, Instant};
    let server = start_server();
    let repo = temp_repo();
    let created = crate::helpers::post_json(
        &format!("{}/api/tasks", server.base),
        &serde_json::json!({"repo_path": repo.path(), "interactive": true,
                            "prompt": "Create the file history.txt containing kept and commit it."}),
    );
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let t0 = Instant::now();
    while crate::helpers::get_json(&format!("{}/api/tasks/{id}", server.base))["activity"] != "waiting" {
        assert!(t0.elapsed() < Duration::from_secs(240), "Claude did not finish its turn");
        std::thread::sleep(Duration::from_millis(500));
    }
    std::thread::sleep(Duration::from_secs(3)); // the copier runs every 2 s
    let kinds = |server: &crate::helpers::Server| {
        let page = crate::helpers::get_json(&format!("{}/api/tasks/{id}/claude?after=0", server.base));
        page["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["kind"].as_str().unwrap().to_owned(), e.to_string()))
            .collect::<Vec<_>>()
    };
    let live = kinds(&server);
    assert!(live.iter().any(|(k, e)| k == "user" && e.contains("history.txt")), "{live:?}");
    assert!(live.iter().any(|(k, _)| k == "tool_use"), "{live:?}");
    crate::helpers::post_json(&format!("{}/api/tasks/{id}/close", server.base), &serde_json::json!({}));
    crate::helpers::wait_for_state(&server, &id, |s| crate::helpers::TERMINAL.contains(&s), Duration::from_secs(90));
    assert!(kinds(&server).len() >= live.len(), "the conversation is gone after the close");
    let usage = crate::helpers::get_json(&format!("{}/api/tasks/{id}/claude/usage", server.base));
    assert!(!usage["samples"].as_array().unwrap().is_empty(), "{usage}");
}
