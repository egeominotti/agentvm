//! The real guest handler in a Python process, through the real HTTP/app/bridge layers.
use crate::helpers::{json_req, send};
use crate::telemetry::vm;
use axum::http::StatusCode;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::test]
async fn structured_control_executes_and_transfers_binary_files_through_the_real_guest_handler() {
    use agentvm::adapters::jobdir::JobWorkspace;
    use agentvm::domain::{ids::TaskId, task::TaskEvent};
    let home = tempfile::tempdir_in("/tmp").unwrap();
    let (ctx, id) = vm(home.path());
    let task = TaskId::parse(&id).unwrap();
    for event in [TaskEvent::SlotAcquired, TaskEvent::Prepared, TaskEvent::VmStarted] {
        ctx.store.apply(&task, event).unwrap();
    }
    let socket = JobWorkspace::pty_socket_of(&ctx.config.jobs(), &task);
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../guest/tests/control_server.py");
    let mut guest = tokio::process::Command::new("python3")
        .arg(script)
        .arg(&socket)
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(guest.stdout.take().unwrap()).lines();
    assert_eq!(lines.next_line().await.unwrap().unwrap(), "ready");
    let app = agentvm::http::router(ctx);
    let exec_path = format!("/api/tasks/{id}/control/exec");
    let (status, started) = send(
        app.clone(),
        json_req("POST", &exec_path, json!({"argv":["/bin/echo","literal ; $(touch BAD)"],"cwd":home.path()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let read_path = format!("{exec_path}/{}", started["exec_id"].as_str().unwrap());
    let result = loop {
        let (status, result) = send(app.clone(), json_req("GET", &read_path, json!({}))).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        if result["state"] != "running" {
            break result;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    assert_eq!(result["stdout"], "bGl0ZXJhbCA7ICQodG91Y2ggQkFEKQo=");
    assert_eq!(result["exit_code"], 0);
    assert!(!home.path().join("BAD").exists());
    let file_path = home.path().join("binary");
    let files = format!("/api/tasks/{id}/control/files");
    let (status, result) = send(
        app.clone(),
        json_req("POST", &files, json!({"op":"write","path":file_path,"data":"AP9B","truncate":true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(std::fs::read(&file_path).unwrap(), b"\x00\xffA");
    let (status, result) =
        send(app.clone(), json_req("POST", &files, json!({"op":"read","path":file_path,"offset":1,"size":2}))).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["data"], "/0E=");
    let (status, result) =
        send(app, json_req("POST", &exec_path, json!({"argv":["/not/a/program"],"cwd":home.path()}))).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{result}");
    guest.kill().await.unwrap();
    guest.wait().await.unwrap();
    std::fs::remove_file(socket).unwrap();
}
