//! A repository given as a link, checked as it is typed: where it is (or will be) cloned, who
//! can read it, and its branches, from a single `ls-remote` (cached for a few minutes).

use super::context::AppCtx;
use super::repos::{RepoCheck, check_folder};
use crate::adapters::git::Git;
use crate::adapters::remote::Visibility;
use crate::domain::branches::remote_heads;
use crate::domain::git_remote::GitRemote;
use crate::domain::ids::RepoPath;

pub fn check_link(ctx: &AppCtx, input: &str) -> RepoCheck {
    let remote = match GitRemote::parse(input) {
        Ok(r) => r,
        Err(e) => {
            let mut answer = check_folder(input);
            answer.ok = false;
            answer.error = Some(e);
            return answer;
        }
    };
    let dir = remote.dir(&ctx.config.home);
    let cloned = dir.join(".git").is_dir();
    let mut answer = if cloned {
        check_folder(&dir.display().to_string())
    } else {
        RepoCheck {
            ok: true,
            path: dir.display().to_string(),
            name: remote.path.rsplit('/').next().unwrap_or_default().to_owned(),
            branch: None,
            sha: None,
            error: None,
            remote: None,
            to_clone: true,
            visibility: None,
            warning: None,
            branches: Vec::new(),
            default_branch: None,
        }
    };
    let heads = match super::remote_repos::visibility(ctx, &remote) {
        Visibility::Public(out) => {
            answer.visibility = Some("public".into());
            Some(remote_heads(&out))
        }
        Visibility::Private(out) => {
            answer.visibility = Some("private".into());
            Some(remote_heads(&out))
        }
        Visibility::NoAccess(why) => {
            answer.visibility = Some("no_access".into());
            if cloned {
                answer.warning = Some(format!(
                    "Out of reach right now ({}): VMs start from the copy on this Mac.",
                    first_line(&why)
                ));
            } else {
                answer.ok = false;
                answer.error =
                    Some(format!("Private, or it does not exist: this Mac has no access. {}", access_hint(&remote)));
            }
            None
        }
    };
    // The remote's branches when it answers; the clone's knowledge of them when it does not.
    match heads {
        Some(h) => {
            answer.default_branch = h.default.or(answer.default_branch);
            answer.branches = h.branches;
        }
        None if cloned => {
            if let Ok(repo) = RepoPath::new(dir) {
                answer.branches = Git::new(repo).remote_branches();
            }
        }
        None => {}
    }
    answer.remote = Some(remote.url);
    answer
}

/// What gives this Mac access to a private repository on `remote`'s host.
fn access_hint(remote: &GitRemote) -> String {
    match remote.host.as_str() {
        "github.com" => "Add a GitHub token in Settings › Git access (fine-grained, Contents: read and write), or run `gh auth login`.".into(),
        "gitlab.com" => "Add a GitLab token in Settings › Git access (scopes read_repository, write_repository).".into(),
        host => format!("Add a token for {host} in Settings › Git access, or an ssh key the host knows."),
    }
}

fn first_line(text: &str) -> &str {
    text.lines().find(|l| !l.trim().is_empty()).unwrap_or(text).trim()
}
