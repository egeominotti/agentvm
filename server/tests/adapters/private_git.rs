//! A private repository: a real git HTTP server on this Mac that refuses anyone without the token
//! (`git http-backend` behind basic auth). Visibility, clone, fetch and push with the token.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use agentvm::adapters::remote::{self, Auth, Visibility};
use agentvm::secret::Secret;

use crate::helpers::sh;

const TOKEN: &str = "github_pat_test_0123456789";
/// Plain http: the test server has no certificate. Production allows only https and ssh.
const TEST: &[&str] = &["http"];

/// `git http-backend` serving `root`, answering 401 without `user:TOKEN`.
struct PrivateServer(Child, u16);

impl Drop for PrivateServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

const SERVER: &str = r#"
import base64, http.server, os, subprocess, sys
root, token = sys.argv[1], sys.argv[2]
want = "Basic " + base64.b64encode(("x-access-token:" + token).encode()).decode()
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def handle_one(self):
        if self.headers.get("Authorization") != want:
            self.send_response(401); self.send_header("WWW-Authenticate", 'Basic realm="git"'); self.end_headers(); return
        path, _, query = self.path.partition("?")
        body = self.rfile.read(int(self.headers.get("Content-Length") or 0))
        env = dict(os.environ, GIT_PROJECT_ROOT=root, GIT_HTTP_EXPORT_ALL="1", PATH_INFO=path, QUERY_STRING=query,
                   REQUEST_METHOD=self.command, CONTENT_TYPE=self.headers.get("Content-Type", ""), REMOTE_USER="x",
                   CONTENT_LENGTH=str(len(body)), GIT_HTTP_RECEIVE_PACK="1")
        out = subprocess.run(["git", "http-backend"], input=body, env=env, capture_output=True).stdout
        head, _, rest = out.partition(b"\r\n\r\n")
        status = 200
        lines = head.decode().split("\r\n")
        for l in lines:
            if l.lower().startswith("status:"): status = int(l.split()[1])
        self.send_response(status)
        for l in lines:
            k, _, v = l.partition(":")
            if k and k.lower() != "status": self.send_header(k, v.strip())
        self.send_header("Content-Length", str(len(rest))); self.end_headers(); self.wfile.write(rest)
    do_GET = do_POST = handle_one
s = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
print(s.server_address[1], flush=True)
s.serve_forever()
"#;

fn private_server(root: &Path) -> PrivateServer {
    let mut child =
        Command::new("python3").args(["-I", "-c", SERVER]).arg(root).arg(TOKEN).stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    PrivateServer(child, line.trim().parse().unwrap())
}

fn private_repo(root: &Path) -> String {
    std::fs::create_dir(root.join("src")).unwrap();
    sh(
        &root.join("src"),
        "git init -q -b main && git config user.email t@t && git config user.name t && echo secret > s && git add . && git commit -qm one",
    );
    sh(root, "git clone -q --bare src shop.git");
    "shop.git".into()
}

fn auth() -> Auth {
    Auth { username: "x-access-token".into(), token: Secret::new(TOKEN.into()) }
}

#[test]
fn a_private_repository_is_told_apart_and_opens_with_the_token() {
    let root = tempfile::tempdir().unwrap();
    let name = private_repo(root.path());
    let server = private_server(root.path());
    let url = format!("http://127.0.0.1:{}/{name}", server.1);

    // Without access: private, and the clone fails at once (git never asks for a password).
    assert!(matches!(remote::visibility(&url, &url, TEST, None), Visibility::NoAccess(_)));
    let dest = root.path().join("clone");
    assert!(remote::clone(&url, &dest, TEST, None).is_err());
    assert!(!dest.exists());

    // With the token: private, and everything works.
    assert_eq!(remote::visibility(&url, &url, TEST, Some(&auth())), Visibility::Private);
    remote::clone(&url, &dest, TEST, Some(&auth())).unwrap();
    assert_eq!(sh(&dest, "cat s"), "secret\n");
    assert_eq!(remote::update(&dest, TEST, Some(&auth())).unwrap(), remote::Update::Current);
    sh(
        &dest,
        "git config user.email a@a && git config user.name a && git checkout -qb agent/x && echo w > w && git add . && git commit -qm work",
    );
    remote::push(&dest, "agent/x", Some(&auth())).unwrap();
    assert_eq!(sh(&root.path().join("shop.git"), "git log -1 --format=%s agent/x"), "work\n");

    // The token is nowhere in the clone's files (not in its remote URL, not in its config).
    let config = std::fs::read_to_string(dest.join(".git/config")).unwrap();
    assert!(!config.contains(TOKEN), "{config}");
}

/// A repository anyone can read is public: no token needed, none used.
#[test]
fn a_repository_without_authentication_is_public() {
    let root = tempfile::tempdir().unwrap();
    private_repo(root.path());
    let url = format!("file://{}", root.path().join("shop.git").display());
    assert_eq!(remote::visibility(&url, &url, &["file"], None), Visibility::Public);
}

/// The real thing, over the network: GitHub says public for a public repository, and nothing for
/// one that does not exist (private and missing look the same to anyone without access).
#[test]
#[ignore = "requires the network (github.com)"]
fn github_tells_public_from_out_of_reach() {
    let public = "https://github.com/octocat/Hello-World.git";
    assert_eq!(remote::visibility(public, public, remote::PROTOCOLS, None), Visibility::Public);
    let missing = "https://github.com/egeominotti/agentvm-no-such-repo-0.git";
    assert!(matches!(remote::visibility(missing, missing, remote::PROTOCOLS, None), Visibility::NoAccess(_)));
}
