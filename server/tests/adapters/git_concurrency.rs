//! Many VMs on one repository finishing at once: each branch gets its own work, never another's.

use std::sync::{Arc, Barrier};

use agentvm::adapters::git::Git;
use agentvm::domain::ids::RepoPath;

use crate::helpers::sh;

#[test]
fn simultaneous_imports_never_swap_work_between_branches() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    sh(&repo, "git init -q -b main && git -c user.name=t -c user.email=t@t commit -q --allow-empty -m base");
    sh(tmp.path(), "git -C repo bundle create -q repo.bundle --all");
    const VMS: usize = 8;
    for round in 0..3 {
        // Each "guest" commits its own file on its own branch and bundles it.
        let bundles: Vec<(String, std::path::PathBuf)> = (0..VMS)
            .map(|i| {
                let (name, guest) = (format!("agent/r{round}-{i}"), tmp.path().join(format!("g{round}-{i}")));
                sh(tmp.path(), &format!("git clone -q repo/repo.bundle {}", guest.display()));
                sh(
                    &guest,
                    &format!(
                        "git checkout -q -b {name} && echo {name} > mine.txt && git add . \
                         && git -c user.name=g -c user.email=g@g commit -qm work \
                         && git bundle create -q out.bundle main..{name}"
                    ),
                );
                (name, guest.join("out.bundle"))
            })
            .collect();
        let start = Arc::new(Barrier::new(VMS));
        let threads: Vec<_> = bundles
            .into_iter()
            .map(|(name, bundle)| {
                let (start, repo) = (start.clone(), repo.clone());
                std::thread::spawn(move || {
                    let git = Git::new(RepoPath::new(repo).unwrap());
                    start.wait();
                    (name.clone(), git.import_bundle(&bundle, &name))
                })
            })
            .collect();
        for t in threads {
            let (name, landed) = t.join().unwrap();
            assert_eq!(landed.as_deref().map_err(|e| e.to_string()), Ok(name.as_str()), "{name}");
            assert_eq!(sh(&repo, &format!("git show {name}:mine.txt")).trim(), name, "{name} got another VM's work");
        }
    }
    assert_eq!(sh(&repo, "git for-each-ref refs/agentvm"), "", "temporary refs left in the user's repository");
}
