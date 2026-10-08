//! Repository bundles shared between launches.

use crate::helpers::git_repo;

/// Twenty VMs on the same repository pack it once: later launches get an instant copy.
#[test]
fn launches_share_one_bundle_per_repository_state() {
    use agentvm::app::bundles::prepare;
    let home = tempfile::tempdir().unwrap();
    let repo = git_repo();
    let (a, b) = (home.path().join("a.bundle"), home.path().join("b.bundle"));
    assert!(!prepare(home.path(), repo.path(), &a).unwrap(), "the first launch packs the repo");
    assert!(prepare(home.path(), repo.path(), &b).unwrap(), "the second one reuses it");
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    // New commits make a new bundle.
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "two"])
            .status()
            .unwrap()
            .success()
    );
    let c = home.path().join("c.bundle");
    assert!(!prepare(home.path(), repo.path(), &c).unwrap());
    assert_ne!(std::fs::read(&a).unwrap(), std::fs::read(&c).unwrap());
}

/// Several launches of the same repository at once, nothing cached yet: none of them deletes
/// the bundle (or the half-written file) another one is about to use.
#[test]
fn simultaneous_first_launches_never_delete_each_others_bundle() {
    use agentvm::app::bundles::prepare;
    for _ in 0..5 {
        let home = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let repo = std::sync::Arc::new(git_repo());
        let start = std::sync::Arc::new(std::sync::Barrier::new(6));
        let launches: Vec<_> = (0..6)
            .map(|i| {
                let (home, repo, start) = (home.clone(), repo.clone(), start.clone());
                std::thread::spawn(move || {
                    let dest = home.path().join(format!("vm{i}.bundle"));
                    start.wait();
                    prepare(home.path(), repo.path(), &dest).map(|_| std::fs::metadata(&dest).unwrap().len())
                })
            })
            .collect();
        for l in launches {
            let size = l.join().unwrap().expect("a launch failed");
            assert!(size > 0);
        }
    }
}
