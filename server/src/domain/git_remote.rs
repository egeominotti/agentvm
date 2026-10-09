//! A repository given as a link (GitHub, GitLab, any git host) instead of a folder: agentvm
//! clones it into a folder of its own and launches from there. A link comes from outside and is
//! handed to git on the Mac, so only plain https and ssh are accepted, with nothing in it that git
//! could read as an option, a local path, credentials or a command (`ext::`).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemote {
    /// What git clones and fetches: https as `https://host/path.git`, ssh as it was typed.
    pub url: String,
    /// `github.com`
    pub host: String,
    /// `acme/shop` (groups included: `group/sub/app`), without `.git`.
    pub path: String,
}

impl GitRemote {
    /// Accepts `https://host/owner/repo[.git]`, `host/owner/repo`, `owner/repo` (GitHub),
    /// `git@host:owner/repo.git` and `ssh://git@host[:port]/owner/repo.git`.
    pub fn parse(link: &str) -> Result<GitRemote, String> {
        let link = link.trim();
        if link.is_empty()
            || link.starts_with('-')
            || link.contains(char::is_whitespace)
            || link.contains(['?', '#', '\\'])
        {
            return Err("Not a repository link: use https://host/owner/repo or git@host:owner/repo.git".into());
        }
        if let Some(rest) = link.strip_prefix("https://") {
            let (host, path) = rest.split_once('/').ok_or("The link has no repository path")?;
            let (host, path) = (host_of(host)?, path_of(path)?);
            return Ok(GitRemote { url: format!("https://{host}/{path}.git"), host, path });
        }
        if let Some(rest) = link.strip_prefix("ssh://") {
            let (authority, path) = rest.split_once('/').ok_or("The link has no repository path")?;
            let (user, host_port) = authority.split_once('@').unwrap_or(("", authority));
            let (host, port) = host_port.split_once(':').unwrap_or((host_port, ""));
            if !user.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
                || !port.chars().all(|c| c.is_ascii_digit())
            {
                return Err("The ssh link's user or port is not valid".into());
            }
            return Ok(GitRemote { url: link.to_owned(), host: host_of(host)?, path: path_of(path)? });
        }
        if link.contains("://") {
            return Err("Only https:// and ssh links can be cloned".into());
        }
        // scp-like ssh: git@github.com:acme/shop.git
        if let Some((user_host, path)) = link.split_once(':') {
            let (user, host) = user_host.split_once('@').ok_or("Not a repository link")?;
            if user.is_empty() || !user.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
                return Err("The ssh link's user is not valid".into());
            }
            return Ok(GitRemote { url: link.to_owned(), host: host_of(host)?, path: path_of(path)? });
        }
        // host/owner/repo, or owner/repo on GitHub.
        let (first, rest) = link.split_once('/').ok_or("Not a repository link")?;
        let (host, path) =
            if first.contains('.') { (host_of(first)?, path_of(rest)?) } else { ("github.com".into(), path_of(link)?) };
        Ok(GitRemote { url: format!("https://{host}/{path}.git"), host, path })
    }

    /// What the repository field holds is a link rather than a folder: folders start with `/`,
    /// `~` or `.`; a link has a scheme, a `user@host:`, a host, or is GitHub's `owner/repo`.
    pub fn looks_like_link(text: &str) -> bool {
        let t = text.trim();
        if t.is_empty() || t.starts_with(['/', '~', '.']) {
            return false;
        }
        if t.contains("://") || (t.contains('@') && t.contains(':')) {
            return true;
        }
        let segments: Vec<&str> = t.trim_end_matches('/').split('/').collect();
        segments.first().is_some_and(|s| s.contains('.')) || segments.len() == 2
    }

    /// Where to open a pull request for `branch` (GitHub, GitLab); `None` elsewhere.
    pub fn pull_request(&self, branch: &str) -> Option<String> {
        match self.host.as_str() {
            "github.com" => Some(format!("https://github.com/{}/compare/{branch}?expand=1", self.path)),
            "gitlab.com" => Some(format!(
                "https://gitlab.com/{}/-/merge_requests/new?merge_request[source_branch]={branch}",
                self.path
            )),
            _ => None,
        }
    }

    /// Where agentvm keeps its clone: `<home>/repos/<host>/<path>`.
    pub fn dir(&self, home: &Path) -> PathBuf {
        self.path.split('/').fold(home.join("repos").join(&self.host), |p, s| p.join(s))
    }
}

fn host_of(host: &str) -> Result<String, String> {
    let host = host.to_ascii_lowercase();
    let valid = host.contains('.')
        && !host.starts_with(['.', '-'])
        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if valid { Ok(host) } else { Err(format!("{host:?} is not a host name (credentials in a link are not accepted)")) }
}

fn path_of(path: &str) -> Result<String, String> {
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments: Vec<&str> = path.split('/').collect();
    let valid = segments.len() >= 2
        && segments.iter().all(|s| {
            !s.is_empty()
                && !s.starts_with(['.', '-'])
                && s.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        });
    if valid { Ok(path.to_owned()) } else { Err("The link needs an owner and a repository: owner/repo".into()) }
}
