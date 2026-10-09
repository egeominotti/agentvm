//! A repository given as a link: which ones are accepted, and where agentvm keeps its clone.

use std::path::Path;

use agentvm::domain::git_remote::GitRemote;

#[test]
fn github_links_in_every_usual_form_are_the_same_repository() {
    for link in [
        "https://github.com/acme/shop",
        "https://github.com/acme/shop.git",
        "https://github.com/acme/shop/",
        "github.com/acme/shop",
        "acme/shop",
        "git@github.com:acme/shop.git",
        "ssh://git@github.com/acme/shop.git",
        "  https://github.com/acme/shop  ",
    ] {
        let r = GitRemote::parse(link).unwrap_or_else(|e| panic!("{link}: {e}"));
        assert_eq!((r.host.as_str(), r.path.as_str()), ("github.com", "acme/shop"), "{link}");
        assert_eq!(r.dir(Path::new("/h")), Path::new("/h/repos/github.com/acme/shop"), "{link}");
    }
    // The form typed is kept: an ssh link clones over ssh (with the Mac's key).
    assert_eq!(GitRemote::parse("git@github.com:acme/shop.git").unwrap().url, "git@github.com:acme/shop.git");
    assert_eq!(GitRemote::parse("acme/shop").unwrap().url, "https://github.com/acme/shop.git");
}

#[test]
fn other_hosts_and_nested_groups_work() {
    let r = GitRemote::parse("https://gitlab.com/group/sub/app.git").unwrap();
    assert_eq!((r.host.as_str(), r.path.as_str()), ("gitlab.com", "group/sub/app"));
    assert_eq!(r.dir(Path::new("/h")), Path::new("/h/repos/gitlab.com/group/sub/app"));
}

/// A link is data from outside, run through git on the Mac: only https and ssh, nothing that
/// git could read as an option, a local path or a command.
#[test]
fn links_that_are_not_plain_https_or_ssh_are_refused() {
    for link in [
        "",
        "/Users/me/code/app",
        "~/code/app",
        "./app",
        "file:///etc",
        "ext::sh -c touch% /tmp/pwned",
        "http://github.com/acme/shop",
        "git://github.com/acme/shop",
        "-uhttps://github.com/acme/shop",
        "https://github.com/acme/../../etc",
        "https://github.com/acme/shop?x=1",
        "https://github.com/",
        "https://github.com/acme",
        "https://user:secret@github.com/acme/shop",
        "git@github.com:-acme/shop",
        "https://git hub.com/acme/shop",
    ] {
        assert!(GitRemote::parse(link).is_err(), "{link:?} was accepted");
    }
}

#[test]
fn a_link_is_told_apart_from_a_folder() {
    assert!(GitRemote::looks_like_link("https://github.com/acme/shop"));
    assert!(GitRemote::looks_like_link("git@github.com:acme/shop.git"));
    assert!(GitRemote::looks_like_link("acme/shop"));
    assert!(!GitRemote::looks_like_link("~/code/shop"));
    assert!(!GitRemote::looks_like_link("/Users/me/shop"));
    assert!(!GitRemote::looks_like_link("code/shop/web"));
}

#[test]
fn a_pushed_branch_links_to_its_pull_request_on_github_and_gitlab() {
    let gh = GitRemote::parse("git@github.com:acme/shop.git").unwrap();
    assert_eq!(gh.pull_request("agent/x").as_deref(), Some("https://github.com/acme/shop/compare/agent/x?expand=1"));
    let gl = GitRemote::parse("https://gitlab.com/group/sub/app").unwrap();
    assert_eq!(
        gl.pull_request("agent/x").as_deref(),
        Some("https://gitlab.com/group/sub/app/-/merge_requests/new?merge_request[source_branch]=agent/x")
    );
    assert_eq!(GitRemote::parse("https://git.example.com/a/b").unwrap().pull_request("agent/x"), None);
}
