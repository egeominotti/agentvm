//! A repository's branches as `git ls-remote --symref <url> HEAD 'refs/heads/*'` lists them, and
//! the names a launch may start from (they reach git as arguments).

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Heads {
    /// Where the remote's HEAD points: the branch a clone checks out.
    pub default: Option<String>,
    /// The default branch first, then the others by name.
    pub branches: Vec<String>,
}

pub fn remote_heads(ls_remote: &str) -> Heads {
    let mut default = None;
    let mut branches = Vec::new();
    for line in ls_remote.lines() {
        if let Some(target) = line.strip_prefix("ref: refs/heads/").and_then(|l| l.strip_suffix("\tHEAD")) {
            default = Some(target.to_owned());
        } else if let Some((_, name)) = line.split_once("\trefs/heads/") {
            branches.push(name.to_owned());
        }
    }
    branches.sort();
    if let Some(d) = &default
        && let Some(i) = branches.iter().position(|b| b == d)
    {
        let main = branches.remove(i);
        branches.insert(0, main);
    }
    Heads { default, branches }
}

/// A plain branch name: what `git check-ref-format --branch` accepts, minus anything git could
/// read as an option or a revision expression.
pub fn is_branch_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with(['-', '/'])
        && !name.ends_with(['/', '.'])
        && !name.ends_with(".lock")
        && name != "@"
        && !name.contains("..")
        && !name.contains("//")
        && !name.contains("@{")
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
}

/// The ref a launch starts from for the branch the user chose (`None`: the default). From a link
/// it is the remote's branch, just fetched; from a folder, the local branch.
pub fn start_ref(branch: Option<&str>, from_link: bool) -> Result<Option<String>, String> {
    let Some(b) = branch.map(str::trim).filter(|b| !b.is_empty()) else { return Ok(None) };
    if !is_branch_name(b) {
        return Err(format!("{b:?} is not a branch name"));
    }
    Ok(Some(if from_link { format!("refs/remotes/origin/{b}") } else { format!("refs/heads/{b}") }))
}
