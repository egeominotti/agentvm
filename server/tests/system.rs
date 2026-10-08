//! Sistema completo: server, VM e Claude veri. Richiede build, golden e token nel Portachiavi.
//! Eseguire con `cargo test --test system -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct Server {
    child: Child,
    base: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_server() -> Server {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bin/agentvm-server");
    let child = Command::new(bin)
        .env("AGENTVM_PORT", port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("bin/agentvm-server: esegui scripts/build.sh");
    let server = Server { child, base: format!("http://127.0.0.1:{port}") };
    let t0 = Instant::now();
    while curl(&["-sf", &format!("{}/api/status", server.base)]).is_none() {
        assert!(t0.elapsed() < Duration::from_secs(10), "il server non risponde");
        std::thread::sleep(Duration::from_millis(100));
    }
    server
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
    std::fs::write(dir.path().join("README.md"), "# prova\n").unwrap();
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
        assert!(t0.elapsed() < max, "timeout, ultimo stato: {task}");
        std::thread::sleep(Duration::from_millis(500));
    }
}

const TERMINAL: [&str; 4] = ["done", "no_changes", "failed", "stopped"];

#[test]
#[ignore = "richiede golden, token e Claude"]
fn task_produces_a_branch_in_the_local_repo() {
    let server = start_server();
    let status = get_json(&format!("{}/api/status", server.base));
    assert_eq!(status["golden"], true);
    assert_eq!(status["token"], true);

    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "prompt": "Crea il file hello.txt con scritto ciao e fai commit."}),
    );
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();

    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(300));
    assert_eq!(task["status"]["state"], "done", "{task}");
    let branch = format!("agent/{id}");
    assert_eq!(task["branch"], branch.as_str());
    assert!(git(repo.path(), &["show", &format!("{branch}:hello.txt")]).to_lowercase().contains("ciao"));
    assert!(curl(&["-sf", &format!("{}/api/tasks/{id}/diff", server.base)]).unwrap().contains("hello.txt"));

    // curl esce con errore allo scadere di --max-time, ma l'output ricevuto è valido.
    let out = Command::new("curl").args(["-sN", "--max-time", "2", &format!("{}/api/tasks/{id}/events", server.base)]).output().unwrap();
    let sse = String::from_utf8_lossy(&out.stdout);
    assert!(sse.contains("event: state") && sse.contains("event: agent") && sse.contains("tool_use"), "{sse}");
}

#[test]
#[ignore = "richiede golden, token e Claude"]
fn running_task_can_be_stopped() {
    let server = start_server();
    let repo = temp_repo();
    let created = post_json(
        &format!("{}/api/tasks", server.base),
        &json!({"repo_path": repo.path(), "prompt": "Esegui il comando `sleep 120` con Bash e poi crea x.txt."}),
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
#[ignore = "richiede golden, token e Claude"]
fn invalid_requests_are_rejected_with_a_message() {
    let server = start_server();
    let not_repo = tempfile::tempdir().unwrap();
    let r = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": not_repo.path(), "prompt": "x"}));
    assert!(r["error"].as_str().unwrap().contains("non è un repository git"), "{r}");
    let repo = temp_repo();
    let r = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "prompt": "  "}));
    assert!(r["error"].as_str().unwrap().contains("vuoto"), "{r}");
    assert!(curl(&["-sf", &format!("{}/api/tasks/nope", server.base)]).is_none());
}

#[test]
#[ignore = "da eseguire dopo gli altri test di sistema"]
fn token_never_lands_in_job_files() {
    let jobs = PathBuf::from(std::env::var("HOME").unwrap()).join("AgentVMs/jobs");
    let out = Command::new("grep").args(["-rl", "sk-ant-"]).arg(&jobs).output().unwrap();
    assert!(out.stdout.is_empty(), "token trovato in: {}", String::from_utf8_lossy(&out.stdout));
}
