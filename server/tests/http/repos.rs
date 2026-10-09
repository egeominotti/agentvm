//! Checking a repository before launching a VM on it.

use axum::http::StatusCode;

use crate::helpers::*;

fn git(dir: &std::path::Path, args: &[&str]) {
    let ok = std::process::Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success();
    assert!(ok, "git {args:?}");
}

async fn check(path: &str) -> serde_json::Value {
    let home = tempfile::tempdir().unwrap();
    let uri = format!("/api/repos/check?path={}", urlencoding(path));
    let req = axum::http::Request::get(uri).header("host", "127.0.0.1:7777").body(axum::body::Body::empty()).unwrap();
    let (status, body) = send(app(home.path()), req).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[tokio::test]
async fn a_repository_says_its_name_branch_and_commit() {
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q", "-b", "trunk"]);
    std::fs::write(repo.path().join("a.txt"), "a").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "first"]);
    let body = check(&repo.path().display().to_string()).await;
    assert_eq!(body["ok"], true, "{body}");
    assert_eq!(body["branch"], "trunk");
    assert_eq!(body["name"], repo.path().file_name().unwrap().to_str().unwrap());
    assert_eq!(body["sha"].as_str().unwrap().len(), 40);
}

#[tokio::test]
async fn a_folder_that_is_not_a_repository_is_said_so() {
    let dir = tempfile::tempdir().unwrap();
    let body = check(&dir.path().display().to_string()).await;
    assert_eq!(body["ok"], false);
    assert!(body["error"].as_str().unwrap().contains("not a git repository"), "{body}");
    let missing = check("/no/such/folder").await;
    assert!(missing["error"].as_str().unwrap().contains("does not exist"), "{missing}");
}

#[tokio::test]
async fn a_repository_without_commits_cannot_be_launched() {
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q"]);
    let body = check(&repo.path().display().to_string()).await;
    assert_eq!(body["ok"], false);
    assert!(body["error"].as_str().unwrap().contains("no commits"), "{body}");
}

/// A shallow clone cannot be bundled whole for the VM: said before launching, with the fix.
#[tokio::test]
async fn a_shallow_clone_is_refused_with_the_fix() {
    let origin = tempfile::tempdir().unwrap();
    git(origin.path(), &["init", "-q", "-b", "main"]);
    for n in ["a", "b"] {
        std::fs::write(origin.path().join(n), n).unwrap();
        git(origin.path(), &["add", "."]);
        git(origin.path(), &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", n]);
    }
    let parent = tempfile::tempdir().unwrap();
    let shallow = parent.path().join("shallow");
    let url = format!("file://{}", origin.path().display());
    let ok = std::process::Command::new("git")
        .args(["clone", "-q", "--depth", "1", &url])
        .arg(&shallow)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let body = check(&shallow.display().to_string()).await;
    assert_eq!(body["ok"], false, "{body}");
    assert!(body["error"].as_str().unwrap().contains("git fetch --unshallow"), "{body}");
}

/// A link says where it would be cloned and who can read it; one this Mac cannot reach (here a
/// host that cannot exist) is refused before launching, with what gives access.
#[tokio::test]
async fn a_link_out_of_reach_says_where_it_would_go_and_how_to_get_access() {
    let body = check("https://agentvm.invalid/acme/shop").await;
    assert_eq!(body["ok"], false, "{body}");
    assert_eq!(body["visibility"], "no_access");
    assert_eq!(body["remote"], "https://agentvm.invalid/acme/shop.git");
    assert_eq!(body["to_clone"], true);
    assert_eq!(body["name"], "shop");
    assert!(body["path"].as_str().unwrap().ends_with("repos/agentvm.invalid/acme/shop"), "{body}");
    assert!(body["error"].as_str().unwrap().contains("Settings › Git access"), "{body}");
    let bad = check("ext::sh -c id").await;
    assert_eq!(bad["ok"], false);
    assert!(bad["error"].is_string(), "{bad}");
}

#[tokio::test]
async fn pushing_the_branch_of_an_unknown_machine_is_not_found() {
    let home = tempfile::tempdir().unwrap();
    let req = axum::http::Request::post("/api/tasks/0199c4b6-a2f2-7fff-bfff-ffffffffffff/push")
        .header("host", "127.0.0.1:7777")
        .body(axum::body::Body::empty())
        .unwrap();
    let (status, _) = send(app(home.path()), req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
