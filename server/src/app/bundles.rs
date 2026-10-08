//! The repository as a git bundle for a VM. Bundles are cached per repository state and handed to
//! each VM as an APFS clone: twenty VMs on the same repo pack it once and share its blocks.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
    // One launch builds a given state; the others wait for it instead of packing it again.
    let building = build_lock(&cached);
    let hit = {
        let _building = building.lock().unwrap();
        let hit = cached.is_file();
        if !hit {
            build(&git, &dir, &cached)?;
        }
        hit
    };
    clone_file(&cached, dest).map_err(|e| e.to_string())?;
    // Only once this launch has its copy, and never the bundle it just used.
    prune(&dir, &repo_key, &cached);
    Ok(hit)
}

/// Packs the repository aside, then publishes it under `cached` without ever replacing a bundle
/// already there: a launch cloning it must never see it swapped (clonefile fails with ENOENT).
fn build(git: &Git, dir: &Path, cached: &Path) -> Result<(), String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = cached.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!("{name}.{}-{n}.tmp", std::process::id()));
    let published =
        git.bundle_all(&tmp).map_err(|e| e.to_string()).and_then(|_| match std::fs::hard_link(&tmp, cached) {
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => Err(e.to_string()),
            _ => Ok(()),
        });
    let _ = std::fs::remove_file(&tmp);
    published
}

/// The lock of one repository state's bundle (forgotten once nobody holds it).
fn build_lock(cached: &Path) -> Arc<Mutex<()>> {
    static LOCKS: Mutex<Option<HashMap<PathBuf, Arc<Mutex<()>>>>> = Mutex::new(None);
    let mut locks = LOCKS.lock().unwrap();
    let locks = locks.get_or_insert_with(HashMap::new);
    locks.retain(|_, lock| Arc::strong_count(lock) > 1);
    locks.entry(cached.to_path_buf()).or_default().clone()
}

/// Keeps the newest finished bundles of `repo_key`. Files still being written by other launches
/// (`.tmp`, git's `.lock`) are theirs, and `keep` is the one just used.
fn prune(dir: &Path, repo_key: &str, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut finished: Vec<_> = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(&format!("{repo_key}-")) && name.ends_with(".bundle")
        })
        .filter(|e| e.path() != keep)
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    finished.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, old) in finished.into_iter().skip(KEEP_PER_REPO - 1) {
        let _ = std::fs::remove_file(old);
    }
}
