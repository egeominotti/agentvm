//! A repository's branches, as git lists them, and which names a launch may start from.

use agentvm::domain::branches::{is_branch_name, remote_heads};

#[test]
fn the_default_branch_comes_first_then_the_others() {
    let out = "ref: refs/heads/main\tHEAD\n\
               aaaa\tHEAD\n\
               bbbb\trefs/heads/dev\n\
               cccc\trefs/heads/feature/login\n\
               aaaa\trefs/heads/main\n";
    let heads = remote_heads(out);
    assert_eq!(heads.default.as_deref(), Some("main"));
    assert_eq!(heads.branches, vec!["main", "dev", "feature/login"]);
}

#[test]
fn a_repository_without_head_or_branches_has_none() {
    assert_eq!(remote_heads("").branches, Vec::<String>::new());
    let heads = remote_heads("bbbb\trefs/heads/b\naaaa\trefs/heads/a\n");
    assert_eq!(heads.default, None);
    assert_eq!(heads.branches, vec!["a", "b"]);
}

#[test]
fn only_plain_branch_names_reach_git() {
    for ok in ["main", "dev", "feature/login", "release-1.2", "fix_bug", "v2"] {
        assert!(is_branch_name(ok), "{ok}");
    }
    for bad in
        ["", "-x", "--upload-pack=sh", "a..b", "a b", "a~1", "a^", "a:b", "/a", "a/", "a.lock", "@", "a//b", "HEAD@{1}"]
    {
        assert!(!is_branch_name(bad), "{bad:?}");
    }
}

/// A chosen branch becomes the ref a launch starts from: the remote's (just fetched) for a link,
/// the local one for a folder. Anything that is not a branch name never reaches git.
#[test]
fn a_chosen_branch_becomes_the_ref_to_start_from() {
    use agentvm::domain::branches::start_ref;
    assert_eq!(start_ref(Some("dev"), true).unwrap().as_deref(), Some("refs/remotes/origin/dev"));
    assert_eq!(start_ref(Some("feature/x"), false).unwrap().as_deref(), Some("refs/heads/feature/x"));
    assert_eq!(start_ref(None, true).unwrap(), None);
    assert_eq!(start_ref(Some("  "), false).unwrap(), None);
    assert!(start_ref(Some("--upload-pack=x"), true).is_err());
}
