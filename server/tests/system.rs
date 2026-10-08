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
        // VMs outlive their server by design: a test that ends (or panics) must not leave its VMs running.
        let _ = Command::new("pkill")
            .args(["-TERM", "-f", &format!("agentvm-vm --config {}", self.home.path().display())])
            .status();
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
    spawn_server_env(home, &[])
}

fn spawn_server_env(home: &Path, env: &[(&str, &str)]) -> (Child, String) {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bin/agentvm-server");
    let child = Command::new(bin)
        .env("AGENTVM_PORT", port.to_string())
        .env("AGENTVM_HOME", home)
        .envs(env.iter().copied())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bin/agentvm-server: run scripts/build.sh");
    (child, format!("http://127.0.0.1:{port}"))
}

fn start_server() -> Server {
    start_server_with(test_home(), &[])
}

fn start_server_with(home: tempfile::TempDir, env: &[(&str, &str)]) -> Server {
    let (child, base) = spawn_server_env(home.path(), env);
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
    let out =
        curl(&["-s", "-X", "POST", "-H", "content-type: application/json", "-d", &body.to_string(), url]).unwrap();
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

/// Runs a command in the VM's root shell (through the real PTY socket) and returns its output.
fn run_in_shell(server: &Server, id: &str, command: &str) -> String {
    let _ = server;
    let socket = agentvm::adapters::jobdir::JobWorkspace::pty_socket_of(
        std::path::Path::new("/"),
        &agentvm::domain::ids::TaskId::parse(id).unwrap(),
    );
    let rt = tokio::runtime::Runtime::new().unwrap();
    // Input sent before the guest's shell exists is lost: wait until it answers.
    let t0 = Instant::now();
    while !rt.block_on(shell_once(&socket, "echo ready-$((1+1))")).contains("ready-2") {
        assert!(t0.elapsed() < Duration::from_secs(60), "the shell never answered");
        std::thread::sleep(Duration::from_millis(300));
    }
    rt.block_on(shell_once(&socket, command))
}

/// Runs `command` once; the marker only appears in the output (tmux replays old screens).
async fn shell_once(socket: &Path, command: &str) -> String {
    use agentvm::adapters::pty::{Frame, PtyConnection};
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let marker = format!("__END_{n}__");
    let Ok(mut pty) = PtyConnection::open(socket, "shell", 200, 50).await else { return String::new() };
    if pty.send(&Frame::Input(format!("clear; {command}; echo __END\"_\"{n}__\n").into_bytes())).await.is_err() {
        return String::new();
    }
    let mut out = String::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline && !out.contains(&marker) {
        if let Ok(Ok(Some(bytes))) = tokio::time::timeout(Duration::from_secs(1), pty.recv()).await {
            out.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    out
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
    let created =
        post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "prompt": "create a.txt"}));
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

fn delete(url: &str) -> u16 {
    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "-X", "DELETE", url])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).parse().unwrap_or(0)
}

#[test]
#[ignore = "needs golden, token and the dev S3 server (scripts/dev-s3.sh up)"]
fn snapshots_go_to_s3_and_come_back() {
    let home = test_home();
    let prefix = format!("system-test-{}", std::process::id());
    let settings = json!({"max_vms": 4, "cpus": 2, "memory_mb": 2048, "timeout_s": 1800, "model": "default",
        "default_repo": null, "claude_version": "latest",
        "s3": {"endpoint": "http://127.0.0.1:9100", "region": "us-east-1", "bucket": "agentvm-backups",
               "prefix": prefix, "access_key": "agentvm", "path_style": true}});
    std::fs::write(home.path().join("settings.json"), settings.to_string()).unwrap();
    let server = start_server_with(home, &[("AGENTVM_S3_SECRET", "agentvm-local-secret")]);
    let base = server.base.clone();
    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "-X", "POST", &format!("{base}/api/settings/s3/test")])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "204", "S3 connection test");

    let repo = temp_repo();
    let created = post_json(&format!("{base}/api/tasks"), &json!({"repo_path": repo.path(), "interactive": true}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    wait_for_state(&server, &id, |s| s == "running", Duration::from_secs(60));
    std::thread::sleep(Duration::from_secs(2));
    let snap = post_json(&format!("{base}/api/tasks/{id}/snapshot"), &json!({"name": "to s3"}));
    let sid = snap["id"].as_str().unwrap_or_else(|| panic!("{snap}")).to_owned();
    post_json(&format!("{base}/api/tasks/{id}/stop"), &json!({}));

    let backed = post_json(&format!("{base}/api/snapshots/{sid}/backup"), &json!({}));
    assert!(backed["archive_mb"].as_u64().unwrap_or(0) > 0, "{backed}");
    assert_eq!(delete(&format!("{base}/api/snapshots/{sid}")), 204);
    let remote = get_json(&format!("{base}/api/backups"));
    let entry = remote
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["snapshot"]["id"] == sid.as_str())
        .unwrap_or_else(|| panic!("{remote}"))
        .clone();
    assert_eq!(entry["local"], false);

    let restored = post_json(&format!("{base}/api/backups/{sid}/restore"), &json!({}));
    assert_eq!(restored["id"], sid.as_str(), "{restored}");
    assert!(get_json(&format!("{base}/api/snapshots")).as_array().unwrap().iter().any(|s| s["id"] == sid.as_str()));

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("snap.tar.zst");
    let out = Command::new("curl")
        .args(["-sf", "-o"])
        .arg(&file)
        .arg(format!("{base}/api/snapshots/{sid}/export"))
        .output()
        .unwrap();
    assert!(out.status.success() && std::fs::metadata(&file).unwrap().len() > 1 << 20, "export");
    let imported = Command::new("curl")
        .args(["-s", "-X", "POST", "--data-binary"])
        .arg(format!("@{}", file.display()))
        .arg(format!("{base}/api/snapshots/import"))
        .output()
        .unwrap();
    let imported: Value = serde_json::from_slice(&imported.stdout).unwrap();
    assert_ne!(imported["id"], sid.as_str(), "{imported}");
    assert_eq!(imported["name"], "to s3");

    assert_eq!(delete(&format!("{base}/api/backups/{sid}")), 204);
    assert!(
        !get_json(&format!("{base}/api/backups"))
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["snapshot"]["id"] == sid.as_str())
    );
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
    let (child, base) = spawn_server(&home_path);
    let again = Server { child, base, home: tempfile::tempdir().unwrap() };
    let t0 = Instant::now();
    while curl(&["-sf", &format!("{}/api/status", again.base)]).is_none() {
        assert!(t0.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(100));
    }
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

/// A repo whose `.agentvm/setup.sh` serves `who.txt` over HTTP on port 3000 and a raw TCP
/// greeting on port 4000, both bound to the VM's own localhost.
fn serving_repo(who: &str) -> tempfile::TempDir {
    let repo = temp_repo();
    std::fs::create_dir_all(repo.path().join(".agentvm")).unwrap();
    std::fs::write(
        repo.path().join(".agentvm/setup.sh"),
        format!(
            "echo {who} > /root/work/who.txt\n\
             nohup python3 -m http.server 3000 --bind 127.0.0.1 --directory /root/work >/dev/null 2>&1 &\n\
             nohup python3 -c \"import socket\ns=socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\n\
             s.bind(('127.0.0.1', 4000)); s.listen()\nwhile True:\n    c, _ = s.accept(); c.sendall(b'tcp-{who}\\\\n'); c.close()\" >/dev/null 2>&1 &\n"
        ),
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "setup"]);
    repo
}

fn wait_for_ports(server: &Server, id: &str) -> Vec<Value> {
    let t0 = Instant::now();
    loop {
        let task = get_json(&format!("{}/api/tasks/{id}", server.base));
        let ports = task["ports"].as_array().cloned().unwrap_or_default();
        if [3000, 4000].iter().all(|p| ports.iter().any(|x| x["port"] == *p)) {
            return ports;
        }
        assert!(t0.elapsed() < Duration::from_secs(90), "ports never appeared: {task}");
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Every VM has its own network: two VMs serve the same ports at once, HTTP ones under their own
/// name `<port>.<vm>.localhost`, the others through a direct TCP forward.
#[test]
#[ignore = "needs golden and token"]
fn vms_serve_the_same_ports_under_their_own_names() {
    let server = start_server();
    let (repo_a, repo_b) = (serving_repo("vm-a"), serving_repo("vm-b"));
    let launch = |repo: &Path| {
        let created =
            post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo, "interactive": true}));
        created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned()
    };
    let (a, b) = (launch(repo_a.path()), launch(repo_b.path()));
    for (id, who) in [(&a, "vm-a"), (&b, "vm-b")] {
        let ports = wait_for_ports(&server, id);
        let http = ports.iter().find(|p| p["port"] == 3000).unwrap();
        assert_eq!(http["kind"], "http", "{http}");
        let url = http["url"].as_str().unwrap_or_else(|| panic!("no url: {http}"));
        let host = url.trim_start_matches("http://").trim_end_matches('/');
        let (name, port) = host.rsplit_once(':').unwrap();
        assert!(name.starts_with("3000.") && name.ends_with(".localhost"), "{url}");
        let body = curl(&[
            "-sf",
            "--max-time",
            "5",
            "--resolve",
            &format!("{name}:{port}:127.0.0.1"),
            &format!("{url}/who.txt"),
        ])
        .unwrap_or_else(|| panic!("{url} does not answer"));
        assert_eq!(body.trim(), who);

        let tcp = ports.iter().find(|p| p["port"] == 4000).unwrap();
        assert_eq!(tcp["kind"], "tcp", "{tcp}");
        let host_port = tcp["host_port"].as_u64().unwrap();
        let mut conn = std::net::TcpStream::connect(("127.0.0.1", host_port as u16)).unwrap();
        let mut greeting = String::new();
        std::io::Read::read_to_string(&mut conn, &mut greeting).unwrap();
        assert_eq!(greeting.trim(), format!("tcp-{who}"));
    }
    // A proxied name never reaches the dashboard's API.
    let fake = format!("3000.nothing-0000.localhost:{}", server.base.rsplit(':').next().unwrap());
    let out = curl(&["-s", "-H", &format!("Host: {fake}"), &format!("{}/api/status", server.base)]).unwrap();
    assert!(!out.contains("golden"), "{out}");
    for id in [&a, &b] {
        post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
        wait_for_state(&server, id, |s| TERMINAL.contains(&s), Duration::from_secs(30));
    }
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

fn put_json(url: &str, body: &Value) -> Value {
    let out = curl(&["-s", "-X", "PUT", "-H", "content-type: application/json", "-d", &body.to_string(), url]).unwrap();
    serde_json::from_str(&out).unwrap()
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
        "Create the file early.txt containing early and commit it. Then run the shell command `sleep 600` \
         in the foreground and wait for it to finish.",
    );
    assert_eq!(task["status"]["state"], "failed", "{task}");
    assert!(task["status"]["reason"].as_str().unwrap_or_default().contains("timeout"), "{task}");
    assert!(git(repo.path(), &["show", &format!("agent/{id}:early.txt")]).contains("early"), "the commit was lost");
}
