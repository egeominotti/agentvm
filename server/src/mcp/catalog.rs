//! Explicit stable tool IDs and route mappings; the generic API fills the remaining JSON surface.
use rmcp::model::{Tool, ToolAnnotations};
use serde_json::{Map, json};

pub struct Route {
    pub name: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub description: &'static str,
    pub destructive: bool,
}

macro_rules! routes {
    ($(($name:literal, $method:literal, $path:literal, $description:literal, $destructive:literal)),* $(,)?) => {
        pub const ROUTES: &[Route] = &[$(Route{name:$name,method:$method,path:$path,description:$description,destructive:$destructive}),*];
    };
}
routes![
    ("agentvm_status", "GET", "/api/status", "Host engine status and memory limits", false),
    ("agentvm_agents", "GET", "/api/agents", "Available agent CLIs, models and configured providers", false),
    ("agentvm_tasks", "GET", "/api/tasks", "List VMs and their current state", false),
    ("agentvm_task", "GET", "/api/tasks/{id}", "Inspect one VM", false),
    ("agentvm_launch", "POST", "/api/tasks", "Launch a VM; body uses the documented launch fields", false),
    ("agentvm_save", "POST", "/api/tasks/{id}/save", "Save a running interactive VM's work to its branch", false),
    ("agentvm_close", "POST", "/api/tasks/{id}/close", "Save and close an interactive VM", false),
    ("agentvm_stop", "POST", "/api/tasks/{id}/stop", "Force-stop a VM, preserving its disk in snapshots", true),
    ("agentvm_delete", "DELETE", "/api/tasks/{id}", "Forget a finished task", true),
    ("agentvm_push", "POST", "/api/tasks/{id}/push", "Push the VM's branch to its repository remote", false),
    ("agentvm_diff", "GET", "/api/tasks/{id}/diff", "Read the VM's repository diff", false),
    ("agentvm_diagnostics", "GET", "/api/tasks/{id}/diagnostics", "Read VM and job diagnostics", false),
    ("agentvm_telemetry", "GET", "/api/tasks/{id}/telemetry", "Read agent runtime telemetry", false),
    (
        "agentvm_output",
        "GET",
        "/api/tasks/{id}/output",
        "Read finite normalized agent/state events with cursor pagination and gap reporting",
        false
    ),
    ("agentvm_history", "GET", "/api/tasks/{id}/claude", "Read available agent conversation history", false),
    ("agentvm_usage", "GET", "/api/tasks/{id}/claude/usage", "Read conversation usage", false),
    ("agentvm_repo_check", "GET", "/api/repos/check", "Check a repository; query contains path", false),
    ("agentvm_snapshots", "GET", "/api/snapshots", "List local snapshots", false),
    ("agentvm_snapshot", "POST", "/api/tasks/{id}/snapshot", "Take a snapshot of a running VM", false),
    (
        "agentvm_snapshot_restore",
        "POST",
        "/api/snapshots/{id}/restore",
        "Restore a local snapshot into a new VM",
        false
    ),
    ("agentvm_snapshot_delete", "DELETE", "/api/snapshots/{id}", "Delete a local snapshot", true),
    ("agentvm_auto_snapshots", "PUT", "/api/tasks/{id}/auto-snapshots", "Configure a VM's automatic snapshots", false),
    ("agentvm_backup", "POST", "/api/snapshots/{id}/backup", "Back up a snapshot to configured S3", false),
    ("agentvm_backups", "GET", "/api/backups", "List remote backups", false),
    ("agentvm_backup_restore", "POST", "/api/backups/{id}/restore", "Restore a remote backup locally", false),
    ("agentvm_backup_delete", "DELETE", "/api/backups/{id}", "Delete a remote backup", true),
    ("agentvm_settings", "GET", "/api/settings", "Read host settings (never credentials)", false),
    ("agentvm_settings_update", "PUT", "/api/settings", "Update host settings", false),
    (
        "agentvm_provider_update",
        "PUT",
        "/api/settings/providers/{id}",
        "Save a provider configuration and optional host-only credential",
        false
    ),
    (
        "agentvm_provider_delete",
        "DELETE",
        "/api/settings/providers/{id}",
        "Delete a provider credential while preserving its endpoint configuration",
        true
    ),
    ("agentvm_storage", "GET", "/api/storage", "Inspect local VM and snapshot storage", false),
    ("agentvm_storage_cleanup", "POST", "/api/storage/cleanup", "Clean unused storage", true),
    ("agentvm_golden", "GET", "/api/golden", "Read golden-image availability and rebuild status", false),
    ("agentvm_golden_rebuild", "POST", "/api/golden/rebuild", "Rebuild the shared golden image", true),
    ("agentvm_versions", "GET", "/api/claude/versions", "List published Claude Code versions", false),
    (
        "agentvm_exec_read",
        "GET",
        "/api/tasks/{id}/control/exec/{exec_id}",
        "Read bounded base64 stdout/stderr and execution status",
        false
    ),
    (
        "agentvm_exec_cancel",
        "DELETE",
        "/api/tasks/{id}/control/exec/{exec_id}",
        "Cancel an execution and kill its process group",
        true
    ),
];

pub fn tool(route: &Route) -> Tool {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for field in ["id", "exec_id"] {
        if route.path.contains(&format!("{{{field}}}")) {
            properties.insert(
                field.into(),
                json!({"type":"string","description":"Explicit VM, snapshot, provider or execution identifier"}),
            );
            required.push(field);
        }
    }
    properties.insert(
        "query".into(),
        json!({"type":"object","additionalProperties":{"type":"string"},"description":"Optional URL query fields"}),
    );
    if route.method != "GET" && route.method != "DELETE" {
        properties.insert(
            "body".into(),
            json!({"type":"object","description":"JSON request body; see agentvm://api resource"}),
        );
    }
    Tool::new(
        route.name,
        route.description,
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
            .as_object()
            .unwrap()
            .clone(),
    )
    .annotate(annotations(route.method == "GET", route.destructive))
}

pub fn annotations(read_only: bool, destructive: bool) -> ToolAnnotations {
    ToolAnnotations::new().read_only(read_only).destructive(destructive).open_world(true)
}

pub fn reference() -> String {
    let mut text =
        String::from("AgentVM JSON API. All identifiers are explicit; the daemon and VMs outlive MCP clients.\n\n");
    for route in ROUTES {
        text.push_str(&format!("{} {} — {}\n", route.method, route.path, route.description));
    }
    text.push_str(include_str!("reference.txt"));
    text
}
