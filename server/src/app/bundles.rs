//! The repository as a git bundle for a VM. Bundles are cached per repository state and handed to
//! each VM as an APFS clone: twenty VMs on the same repo pack it once and share its blocks.

use std::hash::{Hash, Hasher};
use std::path::Path;

use crate::adapters::git::Git;
use crate::adapters::jobdir::clone_file;
use crate::domain::ids::RepoPath;

/// Bundles kept per repository (the newest states).
const KEEP_PER_REPO: usize = 2;

fn hash(text: &str) -> String {
    let mut h = std::hash::DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Writes the bundle of `repo` to `dest`; `true` when it came from the cache.
pub fn prepare(home: &Path, repo: &Path, dest: &Path) -> Result<bool, String> {
    let git = Git::new(RepoPath::new(repo.to_path_buf()).map_err(|e| e.to_string())?);
    let repo_key = hash(&repo.display().to_string());
    let state = hash(&git.refs_fingerprint().map_err(|e| e.to_string())?);
    let dir = home.join("cache").join("bundles");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let cached = dir.join(format!("{repo_key}-{state}.bundle"));
    let hit = cached.is_file();
    if !hit {
        // Built aside and renamed: concurrent launches never see half a bundle.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = dir.join(format!("{repo_key}-{state}.{}-{n}.tmp", std::process::id()));
        git.bundle_all(&tmp).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &cached).map_err(|e| e.to_string())?;
        prune(&dir, &repo_key);
    }
    clone_file(&cached, dest).map_err(|e| e.to_string())?;
    Ok(hit)
}

fn prune(dir: &Path, repo_key: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut mine: Vec<_> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&format!("{repo_key}-")))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    mine.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (_, old) in mine.into_iter().skip(KEEP_PER_REPO) {
        let _ = std::fs::remove_file(old);
    }
}
