//! Translates `claude -p --output-format stream-json` lines into readable events for the dashboard.

use serde::Serialize;
use serde_json::Value;

const SUMMARY_MAX: usize = 300;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEvent {
    Init { model: String, claude_code_version: String },
    Text { text: String },
    ToolUse { name: String, summary: String },
    ToolResult { is_error: bool, summary: String },
    Retry { attempt: u32 },
    Result { is_error: bool, duration_ms: u64, cost_usd: f64, text: String },
    Unparsed { raw: String },
}

/// A line can contain several blocks (text + tool). Unrecognized lines with no useful content
/// (e.g. `rate_limit_event`) produce no events.
pub fn parse_line(line: &str) -> Vec<AgentEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![AgentEvent::Unparsed { raw: clip(line) }];
    };
    let s = |v: &Value| v.as_str().unwrap_or_default().to_owned();
    match (v["type"].as_str(), v["subtype"].as_str()) {
        (Some("system"), Some("init")) => {
            vec![AgentEvent::Init { model: s(&v["model"]), claude_code_version: s(&v["claude_code_version"]) }]
        }
        (Some("system"), Some("api_retry")) => {
            vec![AgentEvent::Retry { attempt: v["attempt"].as_u64().unwrap_or(0) as u32 }]
        }
        (Some("assistant" | "user"), _) => content_blocks(&v["message"]["content"]),
        (Some("result"), _) => vec![AgentEvent::Result {
            is_error: v["is_error"].as_bool().unwrap_or(true),
            duration_ms: v["duration_ms"].as_u64().unwrap_or(0),
            cost_usd: v["total_cost_usd"].as_f64().unwrap_or(0.0),
            text: clip(&s(&v["result"])),
        }],
        _ => Vec::new(),
    }
}

fn content_blocks(content: &Value) -> Vec<AgentEvent> {
    let Some(blocks) = content.as_array() else { return Vec::new() };
    blocks
        .iter()
        .filter_map(|b| match b["type"].as_str()? {
            "text" => Some(AgentEvent::Text { text: clip(b["text"].as_str()?) }),
            "tool_use" => Some(AgentEvent::ToolUse {
                name: b["name"].as_str()?.to_owned(),
                summary: summarize(&tool_input_text(&b["input"])),
            }),
            "tool_result" => Some(AgentEvent::ToolResult {
                is_error: b["is_error"].as_bool().unwrap_or(false),
                summary: summarize(&tool_result_text(&b["content"])),
            }),
            _ => None,
        })
        .collect()
}

/// For the most common tools shows the meaningful field, otherwise the JSON input.
fn tool_input_text(input: &Value) -> String {
    ["command", "file_path", "pattern", "url", "description"]
        .iter()
        .find_map(|k| input[k].as_str().map(str::to_owned))
        .unwrap_or_else(|| input.to_string())
}

fn tool_result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        other => other.to_string(),
    }
}

fn summarize(s: &str) -> String {
    if s.chars().count() <= SUMMARY_MAX { s.to_owned() } else { s.chars().take(SUMMARY_MAX).chain(['…']).collect() }
}

/// Longest text an event keeps (in bytes): the server holds every task's events in memory.
pub const MAX_TEXT: usize = 16 * 1024;

fn clip(s: &str) -> String {
    if s.len() <= MAX_TEXT {
        return s.to_owned();
    }
    let mut end = MAX_TEXT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}
