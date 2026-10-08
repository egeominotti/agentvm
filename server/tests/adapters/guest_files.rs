//! Files the guest controls: symlinks, FIFOs and oversized files never fool the host.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::domain::ids::TaskId;

use crate::helpers::task_id;

/// The guest is root in its VM and can plant symlinks or FIFOs in the shared folder.
#[test]
fn host_requests_never_follow_symlinks_planted_by_the_guest() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(&tmp.path().join("jobs"), &task_id()).unwrap();
    let victim = tmp.path().join("victim.txt");
    std::fs::write(&victim, "precious").unwrap();
    for name in ["save.request", "sync.request", "close.request"] {
        std::os::unix::fs::symlink(&victim, ws.share().join(name)).unwrap();
        agentvm::adapters::jobdir::write_request(&ws.share(), name, "t1").unwrap();
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "precious", "{name} followed the symlink");
        assert!(std::fs::symlink_metadata(ws.share().join(name)).unwrap().is_file());
    }
}

#[test]
fn guest_files_behind_symlinks_fifos_or_too_large_are_not_read() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(&tmp.path().join("jobs"), &task_id()).unwrap();
    let elsewhere = tmp.path().join("result.json");
    std::fs::write(&elsewhere, r#"{"status":"ok","claude_exit":0,"commits":2}"#).unwrap();
    std::os::unix::fs::symlink(&elsewhere, ws.share().join("result.json")).unwrap();
    assert!(ws.read_result().is_none(), "read through a symlink");

    std::fs::write(ws.share().join("metrics.json"), vec![b' '; 8 << 20]).unwrap();
    assert!(ws.read_metrics().is_none(), "read an 8 MiB file");
    std::fs::write(ws.share().join("job.log"), "[1.0s] job start\n".repeat(1 << 20)).unwrap();
    assert!(ws.job_log().iter().map(String::len).sum::<usize>() <= 256 << 10, "job.log read without a cap");

    // A FIFO with no writer would block the reader forever.
    assert!(Command::new("mkfifo").arg(ws.share().join("activity")).status().unwrap().success());
    let share = ws.share();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        tx.send(JobWorkspace::existing(share.parent().unwrap().parent().unwrap(), &task_id_of(&share)).activity())
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).expect("blocked on a FIFO"), None);
}

fn task_id_of(share: &Path) -> TaskId {
    TaskId::parse(&share.parent().unwrap().file_name().unwrap().to_string_lossy()).unwrap()
}

/// Files dropped on a terminal go to `<share>/uploads`, a folder the guest can replace with a
/// symlink to anywhere on the Mac: the host must refuse to write through it.
#[test]
fn uploads_never_land_outside_the_shared_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let share = tmp.path().join("share");
    std::fs::create_dir(&share).unwrap();
    let mut f = agentvm::guestfs::create_in(&share.join("uploads"), "notes.txt").unwrap();
    std::io::Write::write_all(&mut f, b"one").unwrap();
    // Same name again: a new file next to it, never an overwrite.
    let (_, second) = agentvm::guestfs::create_unique(&share.join("uploads"), "notes.txt").unwrap();
    assert_eq!(second, "notes (2).txt");
    assert_eq!(std::fs::read_to_string(share.join("uploads/notes.txt")).unwrap(), "one");

    let elsewhere = tmp.path().join("LaunchAgents");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::remove_dir_all(share.join("uploads")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, share.join("uploads")).unwrap();
    assert!(agentvm::guestfs::create_in(&share.join("uploads"), "evil.plist").is_err());
    assert!(std::fs::read_dir(&elsewhere).unwrap().next().is_none(), "wrote through the symlink");
    for bad in ["../x", "a/b", "..", ".", ""] {
        assert!(agentvm::guestfs::create_unique(&tmp.path().join("u2"), bad).is_err(), "{bad:?}");
    }
}
