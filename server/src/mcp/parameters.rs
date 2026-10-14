//! SDK-generated schemas for control operations and the generic JSON API.
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct ApiArgs {
    pub method: String,
    pub path: String,
    pub body: Option<Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RouteArgs {
    pub id: Option<String>,
    pub exec_id: Option<String>,
    pub query: Option<BTreeMap<String, String>>,
    pub body: Option<Value>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct ExecArgs {
    pub id: String,
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub stdin: Option<String>,
    pub timeout_s: Option<u32>,
    pub output_limit: Option<usize>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct ShellArgs {
    pub id: String,
    /// Explicit script executed as /bin/bash -lc inside the guest.
    pub script: String,
    pub cwd: Option<String>,
    pub stdin: Option<String>,
    pub timeout_s: Option<u32>,
    pub output_limit: Option<usize>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum FileOp {
    List,
    Stat,
    Read,
    Write,
    Mkdir,
    Remove,
    Rename,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct FileArgs {
    pub id: String,
    pub op: FileOp,
    pub path: String,
    pub offset: Option<u64>,
    pub size: Option<usize>,
    /// Base64 binary chunk, at most 1 MiB decoded.
    pub data: Option<String>,
    pub destination: Option<String>,
    pub truncate: Option<bool>,
    pub recursive: Option<bool>,
    pub parents: Option<bool>,
    pub exist_ok: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum TerminalOp {
    List,
    Create,
    Capture,
    Input,
    Resize,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct TerminalArgs {
    pub id: String,
    pub op: TerminalOp,
    pub session: Option<String>,
    pub text: Option<String>,
    pub keys: Option<Vec<String>>,
    pub lines: Option<u32>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub cwd: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct TransferArgs {
    /// export, import, or upload.
    pub operation: TransferOp,
    /// Snapshot ID for export or VM ID for upload.
    pub id: Option<String>,
    /// Explicit absolute file path on the Mac running agentvm-mcp.
    pub local_path: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum TransferOp {
    Export,
    Import,
    Upload,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct OutputArgs {
    pub id: String,
    /// Last consumed event sequence; zero starts from the retained history.
    pub cursor: Option<u64>,
    /// Events per page, 1..500; defaults to 100.
    pub limit: Option<usize>,
}
