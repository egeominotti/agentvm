//! Real SDK stdio and isolated daemon; the MCP client never owns the daemon lifecycle.
use agentvm::adapters::keychain::Keychain;
use agentvm::app::supervisor::AppCtx;
use agentvm::config::Config;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[test]
fn finite_account_import_is_available_through_the_generic_mcp_api() {
    use agentvm::mcp::client::validate_path;
    for account in ["chatgpt", "google", "copilot", "kilo"] {
        assert!(validate_path("POST", &format!("/api/settings/accounts/{account}/import")).is_ok());
    }
    assert!(validate_path("POST", "/api/snapshots/import").is_err());
    assert!(validate_path("GET", "/api/snapshots/example/export").is_err());
}
use tokio::process::Command;

#[test]
fn native_mcp_rejects_remote_urls_and_non_api_routes() {
    use agentvm::mcp::client::{validate_path, validate_url};
    for path in [
        "https://example.com/api/tasks",
        "/",
        "/api/../settings",
        "//evil/api/tasks",
        "/api/tasks#fragment",
        "/api/%2e%2e/settings",
        "/api/tasks\\evil",
        "/api/changes",
        "/api/tasks/id/events",
        "/api/tasks/id/pty",
        "/api/snapshots/id/export",
    ] {
        assert!(validate_path("GET", path).is_err(), "{path}");
    }
    assert!(validate_path("TRACE", "/api/tasks").is_err());
    assert!(validate_path("GET", "/api/tasks?limit=10").is_ok());
    for url in [
        "https://example.com",
        "http://127.0.0.1@evil.test",
        "http://localhost:7777/api",
        "http://127.0.0.1:7777/?secret=x",
    ] {
        assert!(validate_url(url).is_err(), "{url}");
    }
    assert!(validate_url("http://[::1]:7791").is_ok());
}

#[tokio::test]
async fn native_mcp_stdio_discovers_and_calls_real_router_without_stopping_it() {
    let tmp = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = Config {
        home: tmp.path().into(),
        port: listener.local_addr().unwrap().port(),
        concurrency: 1,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 60,
        vm_helper: "agentvm-vm".into(),
        scripts_dir: "scripts".into(),
        min_free_mb: 0,
    };
    let ctx = Arc::new(AppCtx::new(config, Keychain::new(Some(tmp.path().join("none.keychain-db")))));
    let url = format!("http://{}", listener.local_addr().unwrap());
    let snapshot_id = agentvm::domain::snapshot::SnapshotId::generate(std::time::SystemTime::now(), &[0xa5]);
    let snapshot_dir = tmp.path().join("snapshots").join(snapshot_id.as_str());
    std::fs::create_dir_all(&snapshot_dir).unwrap();
    std::fs::write(snapshot_dir.join("disk.raw"), b"\x00\xffMCP binary disk").unwrap();
    std::fs::write(snapshot_dir.join("efivars"), b"efi").unwrap();
    let meta = json!({"id":snapshot_id,"name":"stdio test","source_task":"test","repo":"/r","base_sha":"a".repeat(40),"model":"default","claude_version":null,"created_at":1.0,"size_mb":1});
    std::fs::write(snapshot_dir.join("meta.json"), meta.to_string()).unwrap();
    let router = agentvm::http::router(ctx.clone());
    let daemon = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    for version in ["2026-07-28", "2024-11-05"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_agentvm-mcp"))
            .arg("--url")
            .arg(&url)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        async fn send(input: &mut tokio::process::ChildStdin, value: Value) {
            input.write_all(format!("{value}\n").as_bytes()).await.unwrap();
        }
        let meta = if version == "2026-07-28" {
            json!({"io.modelcontextprotocol/protocolVersion":version,"io.modelcontextprotocol/clientCapabilities":{}})
        } else {
            json!({})
        };
        if version == "2026-07-28" {
            send(&mut input, json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":meta}})).await;
        } else {
            send(&mut input, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":version,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).await;
        }
        let line = tokio::time::timeout(std::time::Duration::from_secs(10), lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        assert!(value.get("error").is_none(), "{value}");
        if version != "2026-07-28" {
            assert_eq!(value["result"]["protocolVersion"], version);
            send(&mut input, json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;
        } else {
            assert!(value["result"]["supportedVersions"].as_array().unwrap().iter().any(|v| v == version));
        }
        send(&mut input, json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":meta}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let tools = value["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|tool| tool["name"] == "agentvm_exec"));
        assert!(tools.iter().any(|tool| tool["name"] == "agentvm_api"));
        assert!(tools.iter().any(|tool| tool["name"] == "agentvm_output"));
        let launch = tools.iter().find(|tool| tool["name"] == "agentvm_launch").unwrap();
        assert_eq!(launch["inputSchema"]["properties"]["repo_path"]["type"], "string");
        let tasks = tools.iter().find(|tool| tool["name"] == "agentvm_tasks").unwrap();
        assert_eq!(tasks["annotations"]["readOnlyHint"], true);
        let stop = tools.iter().find(|tool| tool["name"] == "agentvm_stop").unwrap();
        assert_eq!(stop["annotations"]["destructiveHint"], true);
        send(&mut input, json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_tasks","arguments":{}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_ne!(value["result"]["isError"], true, "{value}");
        assert_eq!(value["result"]["structuredContent"]["data"], json!([]));
        send(&mut input, json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_api","arguments":{"method":"GET","path":"https://evil.test"}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(value["result"]["isError"], true);
        send(
            &mut input,
            json!({"jsonrpc":"2.0","id":5,"method":"resources/read","params":{"_meta":meta,"uri":"agentvm://api"}}),
        )
        .await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert!(value["result"]["contents"][0]["text"].as_str().unwrap().contains("/api/tasks/{id}/control/exec"));
        let archive = tmp.path().join(format!("test-{version}.tar.zst"));
        send(&mut input, json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_transfer","arguments":{"operation":"export","id":snapshot_id,"local_path":archive}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_ne!(value["result"]["isError"], true, "{value}");
        assert!(archive.is_file());
        assert!(value.to_string().len() < 1024, "archive bytes entered tool output");
        send(&mut input, json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_transfer","arguments":{"operation":"export","id":snapshot_id,"local_path":archive}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(value["result"]["isError"], true);
        send(&mut input, json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_transfer","arguments":{"operation":"import","local_path":archive}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_ne!(value["result"]["isError"], true, "{value}");
        let imported = value["result"]["structuredContent"]["data"]["id"].as_str().unwrap();
        assert_eq!(
            std::fs::read(tmp.path().join("snapshots").join(imported).join("disk.raw")).unwrap(),
            b"\x00\xffMCP binary disk"
        );
        send(&mut input, json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_api","arguments":{"method":"PUT","path":"/api/settings/token","body":{"token":"test-host-secret-do-not-echo"}}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert!(!value.to_string().contains("test-host-secret-do-not-echo"));
        use agentvm::domain::ids::{CommitSha, RepoPath, TaskId};
        use agentvm::domain::task::TaskEvent;
        std::fs::create_dir_all(tmp.path().join("repo/.git")).unwrap();
        let task_id = TaskId::generate(std::time::SystemTime::now(), &[0x99]);
        ctx.store.insert(agentvm::app::store::TaskRecord::new(
            task_id.clone(),
            RepoPath::new(tmp.path().join("repo")).unwrap(),
            None,
            CommitSha::parse(&"b".repeat(40)).unwrap(),
            true,
        ));
        for event in [TaskEvent::SlotAcquired, TaskEvent::Prepared, TaskEvent::VmStarted] {
            ctx.store.apply(&task_id, event).unwrap();
        }
        std::fs::create_dir_all(tmp.path().join("jobs").join(task_id.as_str()).join("share")).unwrap();
        let filename = format!("report notes-{version}.bin");
        let upload = tmp.path().join(&filename);
        std::fs::write(&upload, b"\x00\xffliteral upload").unwrap();
        send(&mut input, json!({"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_transfer","arguments":{"operation":"upload","id":task_id,"local_path":upload}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_ne!(value["result"]["isError"], true, "{value}");
        assert_eq!(value["result"]["structuredContent"]["data"]["path"], format!("/mnt/job/uploads/{filename}"));
        assert_eq!(
            std::fs::read(tmp.path().join("jobs").join(task_id.as_str()).join("share/uploads").join(filename)).unwrap(),
            b"\x00\xffliteral upload"
        );
        ctx.store.log(&task_id).unwrap().push(agentvm::app::events::StreamItem::Agent(
            agentvm::domain::agent_event::AgentEvent::Text { text: "MCP finite output".into() },
        ));
        send(&mut input, json!({"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_output","arguments":{"id":task_id,"cursor":0,"limit":1}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_ne!(value["result"]["isError"], true, "{value}");
        assert_eq!(value["result"]["structuredContent"]["data"]["next_cursor"], 1);
        assert_eq!(value["result"]["structuredContent"]["data"]["more"], true);
        send(&mut input, json!({"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_api","arguments":{"method":"GET","path":format!("/api/tasks/{task_id}/output?cursor=1&limit=100")}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_ne!(value["result"]["isError"], true, "{value}");
        assert!(
            value["result"]["structuredContent"]["data"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["item"]["data"]["text"] == "MCP finite output")
        );
        send(&mut input, json!({"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"_meta":meta,"name":"agentvm_api","arguments":{"method":"POST","path":"/api/settings/accounts/claude/import"}}})).await;
        let value: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(value["result"]["isError"], true);
        assert!(value.to_string().contains("native CLI import is unavailable"), "{value}");
        ctx.store.remove(&task_id);
        drop(input);
        tokio::time::timeout(std::time::Duration::from_secs(10), child.wait()).await.unwrap().unwrap();
        assert!(reqwest::get(format!("{url}/api/tasks")).await.unwrap().status().is_success());
    }
    daemon.abort();
}
