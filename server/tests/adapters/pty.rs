//! Terminals: the frame format and the socket each VM's terminals go through.

use std::os::unix::fs::PermissionsExt;

use agentvm::adapters::jobdir::JobWorkspace;

use crate::helpers::task_id;

#[test]
fn pty_frames_are_type_length_payload() {
    use agentvm::adapters::pty::{Frame, encode};
    assert_eq!(encode(&Frame::Input(b"ls\n".to_vec())), [0, 0, 0, 0, 3, b'l', b's', b'\n']);
    let resize = encode(&Frame::Resize { cols: 120, rows: 40 });
    assert_eq!(&resize[..1], &[1]);
    let len = u32::from_be_bytes(resize[1..5].try_into().unwrap()) as usize;
    let body: serde_json::Value = serde_json::from_slice(&resize[5..5 + len]).unwrap();
    assert_eq!(body, serde_json::json!({"cols": 120, "rows": 40}));
}

#[test]
fn pty_socket_path_fits_the_unix_limit_even_for_a_deep_home() {
    let tmp = tempfile::tempdir().unwrap();
    let deep = tmp.path().join("a-very-long-folder-name-for-agentvm-home-that-goes-on-and-on/and/on/jobs");
    let id = task_id();
    let ws = JobWorkspace::create(&deep, &id).unwrap();
    let path = ws.pty_socket();
    assert!(path.as_os_str().len() < 104, "{} bytes: {}", path.as_os_str().len(), path.display());
    assert_eq!(path, JobWorkspace::pty_socket_of(&deep, &id));
    let dir = path.parent().unwrap();
    assert_eq!(std::fs::metadata(dir).unwrap().permissions().mode() & 0o777, 0o700);
}
