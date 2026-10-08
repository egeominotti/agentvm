//! Sistema completo: server, VM e Claude veri. Richiede build, golden e token nel Portachiavi.
//! Eseguire con `cargo test --test system -- --ignored`.

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

/// Ogni server di test ha una propria AGENTVM_HOME (golden collegata), mai quella dell'utente.
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
        .expect("bin/agentvm-server: esegui scripts/build.sh");
    (child, format!("http://127.0.0.1:{port}"))
}

fn start_server() -> Server {
    let home = test_home();
    let (child, base) = spawn_server(home.path());
    let server = Server { child, base, home };
    let t0 = Instant::now();
    while curl(&["-sf", &format!("{}/api/status", server.base)]).is_none() {
        assert!(t0.elapsed() < Duration::from_secs(10), "il server non risponde");
        std::thread::sleep(Duration::from_millis(100));
    }
    server
}

/// Nessun file dei job contiene il token, e nemmeno il branch prodotto.
fn assert_no_token(server: &Server, repo: &Path, branch: Option<&str>) {
    let out = Command::new("grep").args(["-rl", "sk-ant-"]).arg(server.home.path().join("jobs")).output().unwrap();
    assert!(out.stdout.is_empty(), "token trovato in: {}", String::from_utf8_lossy(&out.stdout));
    if let Some(b) = branch {
        let log = git(repo, &["log", "-p", b]);
        assert!(!log.contains("sk-ant-"), "token nel branch {b}");
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
    assert_no_token(&server, repo.path(), Some(&branch));
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

fn run_task(server: &Server, repo: &Path, prompt: &str) -> (String, Value) {
    let created = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo, "prompt": prompt}));
    let id = created["id"].as_str().unwrap_or_else(|| panic!("{created}")).to_owned();
    let task = wait_for_state(server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(300));
    (id, task)
}

#[test]
#[ignore = "richiede golden e bin"]
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
            panic!("il secondo server è partito sulla stessa home");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(!status.success());
    let mut err = String::new();
    std::io::Read::read_to_string(second.stderr.as_mut().unwrap(), &mut err).unwrap();
    assert!(err.contains("già in esecuzione"), "{err}");
    assert!(curl(&["-sf", &format!("{}/api/status", server.base)]).is_some());
}

#[test]
#[ignore = "richiede golden, token e Claude"]
fn prompt_starting_with_a_dash_reaches_claude() {
    let server = start_server();
    let repo = temp_repo();
    let (id, task) = run_task(&server, repo.path(), "- crea il file dash.txt con scritto ok\n- fai commit");
    assert_eq!(task["status"]["state"], "done", "{task}");
    git(repo.path(), &["show", &format!("agent/{id}:dash.txt")]);
}

#[test]
#[ignore = "richiede golden, token e Claude"]
fn commits_on_another_branch_are_not_lost() {
    let server = start_server();
    let repo = temp_repo();
    let (id, task) = run_task(
        &server,
        repo.path(),
        "Esegui `git checkout -b feature/x`, poi crea x.txt con scritto x e fai commit su quel branch.",
    );
    assert_eq!(task["status"]["state"], "done", "{task}");
    git(repo.path(), &["show", &format!("agent/{id}:x.txt")]);
}

#[test]
#[ignore = "richiede golden, token e Claude"]
fn agent_commands_cannot_see_the_token() {
    let server = start_server();
    let repo = temp_repo();
    let (id, task) = run_task(
        &server,
        repo.path(),
        "Esegui `printenv CLAUDE_CODE_OAUTH_TOKEN > env.txt; echo fine >> env.txt` con Bash, poi fai commit di env.txt.",
    );
    assert_eq!(task["status"]["state"], "done", "{task}");
    let content = git(repo.path(), &["show", &format!("agent/{id}:env.txt")]);
    assert!(!content.contains("sk-ant-"), "{content}");
    assert_no_token(&server, repo.path(), Some(&format!("agent/{id}")));
}

#[test]
#[ignore = "richiede golden e token"]
fn stop_right_after_submit_never_leaves_the_task_hanging() {
    let server = start_server();
    let repo = temp_repo();
    let created = post_json(&format!("{}/api/tasks", server.base), &json!({"repo_path": repo.path(), "prompt": "crea a.txt"}));
    let id = created["id"].as_str().unwrap().to_owned();
    post_json(&format!("{}/api/tasks/{id}/stop", server.base), &json!({}));
    let task = wait_for_state(&server, &id, |s| TERMINAL.contains(&s), Duration::from_secs(10));
    assert_eq!(task["status"]["state"], "stopped", "{task}");
    let console = server.home.path().join(format!("jobs/{id}/console.log"));
    assert!(!console.exists() || std::fs::metadata(&console).unwrap().len() == 0, "la VM è partita comunque");
}
