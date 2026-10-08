//! Full system: real server, VM and Claude. Requires the build, the golden image and the token in the Keychain.
//! Run with `cargo test --test system -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct Server {
    child: Child,
    base: String,
    home: tempfile::TempDir,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Each test server has its own AGENTVM_HOME (with the golden image linked), never the user's.
fn test_home() -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("golden")).unwrap();
    let golden = PathBuf::from(std::env::var("HOME").unwrap()).join("AgentVMs/golden/disk.raw");
    std::os::unix::fs::symlink(golden, home.path().join("golden/disk.raw")).unwrap();
    home
}

fn spawn_server(home: &Path) -> (Child, String) {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bin/agentvm-server");
    let child = Command::new(bin)
        .env("AGENTVM_PORT", port.to_string())
        .env("AGENTVM_HOME", home)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bin/agentvm-server: run scripts/build.sh");
    (child, format!("http://127.0.0.1:{port}"))
}

fn start_server() -> Server {
    let home = test_home();
    let (child, base) = spawn_server(home.path());
    let server = Server { child, base, home };
    let t0 = Instant::now();
    while curl(&["-sf", &format!("{}/api/status", server.base)]).is_none() {
        assert!(t0.elapsed() < Duration::from_secs(10), "the server is not responding");
        std::thread::sleep(Duration::from_millis(100));
    }
    server
}

/// No job file contains the token, and neither does the produced branch.
fn assert_no_token(server: &Server, repo: &Path, branch: Option<&str>) {
    let out = Command::new("grep").args(["-rl", "sk-ant-"]).arg(server.home.path().join("jobs")).output().unwrap();
    assert!(out.stdout.is_empty(), "token found in: {}", String::from_utf8_lossy(&out.stdout));
    if let Some(b) = branch {
        let log = git(repo, &["log", "-p", b]);
        assert!(!log.contains("sk-ant-"), "token in branch {b}");
    }
}

fn curl(args: &[&str]) -> Option<String> {
    let out = Command::new("curl").args(args).output().unwrap();
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn get_json(url: &str) -> Value {
    serde_json::from_str(&curl(&["-sf", url]).unwrap_or_else(|| panic!("GET {url}"))).unwrap()
}

fn post_json(url: &str, body: &Value) -> Value {
    let out = curl(&["-s", "-X", "POST", "-H", "content-type: application/json", "-d", &body.to_string(), url]).unwrap();
    serde_json::from_str(&out).unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn temp_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "t@t"]);
    git(dir.path(), &["config", "user.name", "t"]);
    std::fs::write(dir.path().join("README.md"), "# test\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "init"]);
    dir
}

fn wait_for_state(server: &Server, id: &str, done: impl Fn(&str) -> bool, max: Duration) -> Value {
    let t0 = Instant::now();
    loop {
        let task = get_json(&format!("{}/api/tasks/{id}", server.base));
        if done(task["status"]["state"].as_str().unwrap()) {
            return task;
        }
        assert!(t0.elapsed() < max, "timeout, last state: {task}");
        std::thread::sleep(Duration::from_millis(500));
    }
}

const TERMINAL: [&str; 4] = ["done", "no_changes", "failed", "stopped"];

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
    let out = Command::new("curl").args(["-sN", "--max-time", "2", &format!("{}/api/tasks/{id}/events", server.base)]).output().unwrap();
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

fn run_task(server: &Server, repo: &Path, prompt: &str) -> (String, Value) {
    let created = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo, "prompt": prompt}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let task = wait_for_state(server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(300));
    (id, task)
}

#[test]
#[ignore = "requires golden and bin"]
fn second_server_on_the_same_home_refuses_to_start() {
    let server = start_server();
    let (mut second, _) = spawn_server(server.home.path());
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
#[ignore = "requires golden, token and Claude"]
fn agent_commands_cannot_see_the_token() {
    let server = start_server();
    let repo = temp_repo();
    let (id, task) = run_task(
        &server,
        repo.path(),
        "Run `printenv CLAUDE_CODE_OAUTH_TOKEN > env.txt; echo end >> env.txt` with Bash, then commit env.txt.",
    );
    assert_eq!(task["status"]["state"], "done", "{task}");
    let content = git(repo.path(), &["show", &format!("agent/{id}:env.txt")]);
    assert!(!content.contains("sk-ant-"), "{content}");
    assert_no_token(&server, repo.path(), Some(&format!("agent/{id}")));
}

#[test]
#[ignore = "requires golden and token"]
fn stop_right_after_submit_never_leaves_the_task_hanging() {
    let server = start_server();
    let repo = temp_repo();
    let created = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "prompt": "create a.txt"}));
    let id = created["id"].as_str().unwrap().to_owned();
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(10));
    assert_eq!(task["status"]["state"], "stopped", "{task}");
    let console = server.home.path().join(format!("jobs/{id}/console.log"));
    assert!(!console.exists() || std::fs::metadata(&console).unwrap().len() == 0, "the VM started anyway");
}

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
    let other = releases["versions"].as_array().unwrap().iter().filter_map(|v| v.as_str()).find(|v| *v != latest).unwrap().to_owned();
    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "claude_version": other, "prompt": "Reply with the single word ok."}),
    );
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(300));
    assert_ne!(task["status"]["state"], "failed", "{task}");
    let out = Command::new("curl").args(["-sN", "--max-time", "2", &format!("{}/api/tasks/{id}/events", server.base)]).output().unwrap();
    let sse = String::from_utf8_lossy(&out.stdout);
    assert!(sse.contains(&format!("\"claude_code_version\":\"{other}\"")), "wanted {other}: {sse}");
}
