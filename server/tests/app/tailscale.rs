//! Tailscale: a VM joins the user's tailnet only with an auth key saved.

use agentvm::app::submission::{NewTask, SubmitError, submit};

use agentvm::app::tailscale::{TailscaleError, join, leave};
use agentvm::domain::ids::TaskId;

use crate::helpers::{ctx, git_repo, running_task};

/// A launch that asks for the tailnet with no key saved is refused up front, saying what to do,
/// instead of a VM that boots and silently stays off the tailnet.
#[tokio::test]
async fn joining_the_tailnet_needs_a_saved_key() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let ctx = ctx(home.path());
    let repo = repo.path().display().to_string();
    let req = NewTask {
        repo: &repo,
        prompt: String::new(),
        base_ref: None,
        interactive: true,
        model: None,
        claude_version: None,
        restore_from: None,
        cpus: Some(1),
        memory_mb: Some(1024),
        label: None,
        tailscale: Some(true),
    };
    let err = submit(&ctx, req).unwrap_err();
    assert!(matches!(err, SubmitError::NoTailscaleKey), "{err}");
    assert!(err.to_string().contains("Settings"), "{err}");
    assert!(ctx.store.list().is_empty());
}

/// A key that is not a Tailscale key never reaches the Keychain.
#[tokio::test]
async fn only_a_tailscale_key_can_be_saved() {
    let home = tempfile::tempdir().unwrap();
    let ctx = ctx(home.path());
    let err =
        agentvm::app::tailscale::save_key(&ctx, agentvm::secret::Secret::new("sk-ant-oat01-nope".into())).unwrap_err();
    assert!(err.to_string().contains("tskey-"), "{err}");
    assert!(!agentvm::app::tailscale::key_saved(&ctx));
}

/// Joining from a VM's page: only a running terminal can, and only with a key saved.
#[tokio::test]
async fn only_a_running_terminal_with_a_key_saved_joins() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let unknown = TaskId::parse("20261008-185855-4f94").unwrap();
    assert!(matches!(join(&ctx(home.path()), &unknown).await, Err(TailscaleError::NotFound)));

    let batch = running_task(home.path(), &repo, false);
    let ctx = ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    let err = join(&ctx, &batch).await.unwrap_err();
    assert!(matches!(err, TailscaleError::NotRunning), "{err}");

    let home = tempfile::tempdir().unwrap();
    let term = running_task(home.path(), &repo, true);
    let ctx = crate::helpers::ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    let err = join(&ctx, &term).await.unwrap_err();
    assert!(matches!(err, TailscaleError::NoKey), "{err}");
    assert!(err.to_string().contains("Settings"), "{err}");
    assert!(!ctx.store.get(&term).unwrap().tailscale, "a refused join leaves the machine as it was");
}

/// Leaving the tailnet from a VM that is not running is refused, not silently "done".
#[tokio::test]
async fn leaving_needs_a_running_terminal() {
    let home = tempfile::tempdir().unwrap();
    let unknown = TaskId::parse("20261008-185855-4f94").unwrap();
    assert!(matches!(leave(&ctx(home.path()), &unknown).await, Err(TailscaleError::NotFound)));
}

/// A VM launched without Tailscale and joined from its page: what its guest reports reaches the
/// dashboard (it used to be read only for VMs that joined at launch).
#[tokio::test]
async fn a_vm_joined_after_launch_shows_its_tailnet() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let id = running_task(home.path(), &repo, true);
    let ctx = ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    let share = agentvm::adapters::jobdir::JobWorkspace::share_of(&home.path().join("jobs"), &id);
    std::fs::create_dir_all(&share).unwrap();
    std::fs::write(share.join("tailscale.json"), r#"{"name":"agentvm-x-4f94.tail1.ts.net","ips":["100.64.0.5"]}"#)
        .unwrap();

    agentvm::app::tailscale::refresh(&ctx, &id);
    assert_eq!(ctx.store.get(&id).unwrap().tailnet, None, "not asked to join: nothing shown");
    ctx.store.set_tailscale(&id, true);
    agentvm::app::tailscale::refresh(&ctx, &id);
    let tailnet = ctx.store.get(&id).unwrap().tailnet.expect("joined after launch, and shown");
    assert_eq!(tailnet.name.as_deref(), Some("agentvm-x-4f94.tail1.ts.net"));
}
