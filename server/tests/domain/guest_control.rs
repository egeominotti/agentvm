//! Guest wire input is validated without any host or VM I/O.
use agentvm::domain::guest_control::validate;
use serde_json::json;

#[test]
fn guest_control_accepts_empty_file_chunks_for_empty_files() {
    assert!(validate("files.write", &json!({"path":"/root/work/empty","data":"","truncate":true})).is_ok());
}

#[test]
fn guest_control_rejects_invalid_execution_limits_and_terminal_names() {
    for args in [
        json!({"argv":[]}),
        json!({"argv":["true"],"timeout_s":0}),
        json!({"argv":["true"],"output_limit":1048577}),
        json!({"argv":["true"],"stdin":4}),
    ] {
        assert!(validate("exec.start", &args).is_err(), "{args}");
    }
    assert!(validate("terminal.input", &json!({"session":"shell-10","text":"hello"})).is_err());
    assert!(validate("files.read", &json!({"path":"x","size":1048577})).is_err());
}
