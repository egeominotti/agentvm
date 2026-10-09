//! Claude Code's session files (`~/.claude/projects/*/*.jsonl`), read into the conversation the
//! dashboard shows: prompts, Claude's messages, each tool call and its result, tokens per turn.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::agent_event::clip;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum EntryKind {
    User { text: String },
    Assistant { text: String },
    ToolUse { name: String, input: String },
    ToolResult { is_error: bool, output: String },
}

/// The tokens of one model call (repeated on each block of the same message: count it once
/// per `message_id`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct TurnUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct HistoryEntry {
    /// ISO 8601, as Claude Code wrote it.
    pub at: String,
    /// Written by a subagent rather than the main conversation.
    pub sidechain: bool,
    #[serde(flatten)]
    pub kind: EntryKind,
    pub message_id: Option<String>,
    pub usage: Option<TurnUsage>,
}

/// The entries of one line of a session file; lines that are not part of the conversation (and
/// Claude's private thinking) give none.
pub fn parse_line(line: &str) -> Vec<HistoryEntry> {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return Vec::new() };
    let role = match v["type"].as_str() {
        Some(r @ ("user" | "assistant")) => r,
        _ => return Vec::new(),
    };
    let message = &v["message"];
    let entry = |kind| HistoryEntry {
        at: v["timestamp"].as_str().unwrap_or_default().to_owned(),
        sidechain: v["isSidechain"].as_bool().unwrap_or(false),
        kind,
        message_id: message["id"].as_str().map(str::to_owned),
        usage: serde_json::from_value(message["usage"].clone()).ok().filter(|u: &TurnUsage| *u != TurnUsage::default()),
    };
    if let Some(text) = message["content"].as_str() {
        return vec![entry(EntryKind::User { text: clip(text) })];
    }
    let blocks = message["content"].as_array().map(Vec::as_slice).unwrap_or_default();
    blocks
        .iter()
        .filter_map(|b| match (b["type"].as_str()?, role) {
            ("text", "user") => Some(EntryKind::User { text: clip(b["text"].as_str()?) }),
            ("text", _) => Some(EntryKind::Assistant { text: clip(b["text"].as_str()?) }),
            ("tool_use", _) => {
                Some(EntryKind::ToolUse { name: b["name"].as_str()?.to_owned(), input: clip(&tool_input(&b["input"])) })
            }
            ("tool_result", _) => Some(EntryKind::ToolResult {
                is_error: b["is_error"].as_bool().unwrap_or(false),
                output: clip(&tool_output(&b["content"])),
            }),
            _ => None,
        })
        .map(entry)
        .collect()
}

/// The meaningful field of the common tools, the whole input otherwise.
fn tool_input(input: &Value) -> String {
    ["command", "file_path", "pattern", "url", "prompt"]
        .iter()
        .find_map(|k| input[k].as_str().map(str::to_owned))
        .unwrap_or_else(|| input.to_string())
}

fn tool_output(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}
