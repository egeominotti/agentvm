//! Ports served inside each VM, reachable from the Mac under the VM's own name.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::helpers::{Server, TERMINAL, curl, get_json, git, post_json, start_server, temp_repo, wait_for_state};

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
