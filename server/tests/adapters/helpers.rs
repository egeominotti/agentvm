//! Shell commands and task ids shared by the adapter tests.

use std::path::Path;
use std::process::Command;

use agentvm::domain::ids::TaskId;

pub(crate) fn sh(dir: &Path, cmd: &str) -> String {
    let out = Command::new("sh").arg("-c").arg(cmd).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "{cmd}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

pub(crate) fn task_id() -> TaskId {
    TaskId::generate(std::time::SystemTime::now(), [0xab, 0xcd])
}
