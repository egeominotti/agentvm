//! Requests to a VM through its shared folder: each reply goes to the request it answers.

use std::path::{Path, PathBuf};
use std::time::Duration;

use agentvm::app::session::{Saved, save};

use crate::helpers::{ctx, git_repo, running_task};

/// The guest's request loop: answers every `save.request` with `reply(id)`, for `rounds` requests.
fn guest_saves(share: &Path, rounds: usize, reply: impl Fn(&str) -> Vec<String> + Send + 'static) {
    let share = share.to_path_buf();
    std::thread::spawn(move || {
        let mut served = 0;
        for _ in 0..1500 {
            if let Ok(id) = std::fs::read_to_string(share.join("save.request")) {
                std::fs::remove_file(share.join("save.request")).unwrap();
                for json in reply(id.trim()) {
                    write_reply(&share, "save.done", &json);
                    std::thread::sleep(Duration::from_millis(300));
                }
                served += 1;
                if served == rounds {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
}

/// Atomic, as the guest's `reply` does it (temporary file, then rename).
fn write_reply(share: &Path, name: &str, json: &str) {
    let tmp: PathBuf = share.join(format!("{name}.tmp"));
    std::fs::write(&tmp, json).unwrap();
    std::fs::rename(tmp, share.join(name)).unwrap();
}

fn terminal(
    home: &Path,
    repo: &tempfile::TempDir,
) -> (std::sync::Arc<agentvm::app::supervisor::AppCtx>, agentvm::domain::ids::TaskId, PathBuf) {
    let id = running_task(home, repo, true);
    let ctx = ctx(home);
    ctx.store.insert(agentvm::app::store::Store::load(&home.join("jobs")).remove(0));
    let share = home.join("jobs").join(id.as_str()).join("share");
    std::fs::create_dir_all(&share).unwrap();
    (ctx, id, share)
}

/// A reply that arrives late, for an earlier save the host gave up on, is not this save's answer.
#[tokio::test(flavor = "multi_thread")]
async fn a_late_reply_to_an_earlier_save_is_ignored() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let (ctx, id, share) = terminal(home.path(), &repo);
    guest_saves(&share, 1, |id| {
        vec![
            r#"{"id":"an-earlier-save","commits":0,"error":"stale"}"#.into(),
            format!(r#"{{"id":"{id}","commits":0}}"#),
        ]
    });
    let saved = save(&ctx, &id).await.unwrap();
    assert_eq!(saved, Saved { commits: 0, branch: id.branch() });
}

/// Two saves of one VM at the same moment are served one after the other, each with its answer.
#[tokio::test(flavor = "multi_thread")]
async fn simultaneous_saves_of_one_vm_each_get_their_answer() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let (ctx, id, share) = terminal(home.path(), &repo);
    guest_saves(&share, 2, |id| vec![format!(r#"{{"id":"{id}","commits":0}}"#)]);
    let (a, b) = tokio::join!(save(&ctx, &id), save(&ctx, &id));
    assert!(a.is_ok() && b.is_ok(), "{a:?} {b:?}");
}
