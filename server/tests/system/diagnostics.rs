//! A real VM that fails: its diagnostics name the cause and carry the evidence.

use std::time::Duration;

use serde_json::json;

use crate::helpers::{TERMINAL, get_json, git, post_json, start_server, temp_repo, wait_for_state};

/// The repository's setup.sh fails, then Claude fails too (a model that does not exist): the
/// diagnostics point at setup.sh and hold its output, the timeline ends in `failed`, and the
/// server log has this VM's lines.
#[test]
#[ignore = "needs golden and token"]
fn a_failed_vm_says_why_in_its_diagnostics() {
    let server = start_server();
    let repo = temp_repo();
    std::fs::create_dir_all(repo.path().join(".agentvm")).unwrap();
    std::fs::write(repo.path().join(".agentvm/setup.sh"), "echo 'setup broke: missing tool' >&2\nexit 1\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "setup"]);
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "prompt": "Say hello.", "model": "claude-does-not-exist-9"}),
    );
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(240));
    assert_eq!(task["status"]["state"], "failed", "{task}");

    let d = get_json(&format!("{}/api/tasks/{id}/diagnostics", server.base));
    assert!(d["hint"].as_str().unwrap_or_default().contains("setup.sh"), "{d}");
    let setup = d["logs"].as_array().unwrap().iter().find(|l| l["file"] == "share/setup.log");
    assert!(setup.is_some_and(|l| l["tail"].as_str().unwrap().contains("setup broke")), "{d}");
    assert_eq!(d["timeline"].as_array().unwrap().last().unwrap()["state"], "failed", "{d}");
    assert!(d["server_log"].as_array().unwrap().iter().any(|l| l.as_str().unwrap().contains("to=failed")), "{d}");
}
