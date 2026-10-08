//! A real server on its own home, and the curl, git and shell helpers the tests drive it with.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub(crate) struct Server {
    pub(crate) child: Child,
    pub(crate) base: String,
    pub(crate) home: tempfile::TempDir,
    /// Everything the server wrote on stderr, shown when the test fails.
    log: Arc<Mutex<String>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // VMs outlive their server by design: a test that ends (or panics) must not leave its VMs running.
        let _ = Command::new("pkill")
            .args(["-TERM", "-f", &format!("agentvm-vm --config {}", self.home.path().display())])
            .status();
        if std::thread::panicking() {
            // Keep the jobs (Claude's stream.jsonl, guest logs) to find out why.
            self.home.disable_cleanup(true);
            eprintln!("server home kept: {}\nserver log:\n{}", self.home.path().display(), self.log.lock().unwrap());
        }
    }
}

/// Each test server has its own AGENTVM_HOME (with the golden image linked), never the user's.
pub(crate) fn test_home() -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("golden")).unwrap();
    let golden = PathBuf::from(std::env::var("HOME").unwrap()).join("AgentVMs/golden/disk.raw");
    std::os::unix::fs::symlink(golden, home.path().join("golden/disk.raw")).unwrap();
    home
}

/// A server that may fail to start: its stderr is left to the caller.
pub(crate) fn spawn_server(home: &Path) -> Child {
    Command::new(server_bin())
        .env("AGENTVM_PORT", "0")
        .env("AGENTVM_HOME", home)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bin/agentvm-server: run scripts/build.sh")
}

fn server_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bin/agentvm-server")
}

/// Starts a server on a port it picks itself (no race with other tests for a free port) and
/// keeps reading its stderr, so a chatty server never blocks on a full pipe.
fn launch(home: tempfile::TempDir, env: &[(&str, &str)]) -> Server {
    let path = home.path().to_path_buf();
    launch_at(&path, home, env)
}

/// `home` is only owned (deleted at the end) by the returned server; the server runs on `path`.
pub(crate) fn launch_at(path: &Path, home: tempfile::TempDir, env: &[(&str, &str)]) -> Server {
    let mut child = Command::new(server_bin())
        .env("AGENTVM_PORT", "0")
        .env("AGENTVM_HOME", path)
        .envs(env.iter().copied())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bin/agentvm-server: run scripts/build.sh");
    let stderr = child.stderr.take().unwrap();
    let log = Arc::new(Mutex::new(String::new()));
    let (tx, rx) = std::sync::mpsc::channel();
    let sink = log.clone();
    std::thread::spawn(move || {
        for line in std::io::BufRead::lines(std::io::BufReader::new(stderr)).map_while(Result::ok) {
            if let Some(url) = line.strip_prefix("agentvm listening on ") {
                let _ = tx.send(url.to_owned());
            }
            let mut log = sink.lock().unwrap();
            log.push_str(&line);
            log.push('\n');
        }
    });
    let started = rx.recv_timeout(Duration::from_secs(30));
    let server = Server { child, base: started.clone().unwrap_or_default(), home, log };
    assert!(started.is_ok(), "the server did not start:\n{}", server.log.lock().unwrap());
    let t0 = Instant::now();
    while curl(&["-sf", &format!("{}/api/status", server.base)]).is_none() {
        assert!(t0.elapsed() < Duration::from_secs(10), "the server is not responding");
        std::thread::sleep(Duration::from_millis(100));
    }
    server
}

pub(crate) fn start_server() -> Server {
    start_server_with(test_home(), &[])
}

pub(crate) fn start_server_with(home: tempfile::TempDir, env: &[(&str, &str)]) -> Server {
    launch(home, env)
}

/// No job file contains the token, and neither does the produced branch.
pub(crate) fn assert_no_token(server: &Server, repo: &Path, branch: Option<&str>) {
    let out = Command::new("grep").args(["-rl", "sk-ant-"]).arg(server.home.path().join("jobs")).output().unwrap();
    assert!(out.stdout.is_empty(), "token found in: {}", String::from_utf8_lossy(&out.stdout));
    if let Some(b) = branch {
        let log = git(repo, &["log", "-p", b]);
        assert!(!log.contains("sk-ant-"), "token in branch {b}");
    }
}

pub(crate) fn curl(args: &[&str]) -> Option<String> {
    let out = Command::new("curl").args(args).output().unwrap();
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub(crate) fn get_json(url: &str) -> Value {
    serde_json::from_str(&curl(&["-sf", url]).unwrap_or_else(|| panic!("GET {url}"))).unwrap()
}

pub(crate) fn post_json(url: &str, body: &Value) -> Value {
    let out =
        curl(&["-s", "-X", "POST", "-H", "content-type: application/json", "-d", &body.to_string(), url]).unwrap();
    serde_json::from_str(&out).unwrap()
}

pub(crate) fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub(crate) fn temp_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "t@t"]);
    git(dir.path(), &["config", "user.name", "t"]);
    std::fs::write(dir.path().join("README.md"), "# test\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "init"]);
    dir
}

pub(crate) fn wait_for_state(server: &Server, id: &str, done: impl Fn(&str) -> bool, max: Duration) -> Value {
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

pub(crate) const TERMINAL: [&str; 4] = ["done", "no_changes", "failed", "stopped"];

/// Runs a command in the VM's root shell (through the real PTY socket) and returns its output.
pub(crate) fn run_in_shell(server: &Server, id: &str, command: &str) -> String {
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

pub(crate) fn run_task(server: &Server, repo: &Path, prompt: &str) -> (String, Value) {
    let created = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo, "prompt": prompt}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let task = wait_for_state(server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(300));
    (id, task)
}

pub(crate) fn delete(url: &str) -> u16 {
    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "-X", "DELETE", url])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).parse().unwrap_or(0)
}

pub(crate) fn put_json(url: &str, body: &Value) -> Value {
    let out = curl(&["-s", "-X", "PUT", "-H", "content-type: application/json", "-d", &body.to_string(), url]).unwrap();
    serde_json::from_str(&out).unwrap()
}
