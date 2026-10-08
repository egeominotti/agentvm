//! Claude Code's session files, read into the conversation the dashboard shows.

use agentvm::domain::transcript::{EntryKind, parse_line};
use serde_json::json;

fn line(v: serde_json::Value) -> String {
    v.to_string()
}

#[test]
fn a_prompt_is_a_user_entry() {
    let e = parse_line(&line(json!({"type": "user", "timestamp": "2026-10-09T10:00:00Z", "isSidechain": false,
        "message": {"role": "user", "content": "Fix the parser"}})));
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].kind, EntryKind::User { text: "Fix the parser".into() });
    assert_eq!(e[0].at, "2026-10-09T10:00:00Z");
}

#[test]
fn assistant_blocks_carry_their_message_and_usage() {
    let usage = json!({"input_tokens": 12, "output_tokens": 340, "cache_read_input_tokens": 5000, "cache_creation_input_tokens": 0});
    let text = parse_line(&line(
        json!({"type": "assistant", "timestamp": "t1", "message": {"id": "msg_1", "role": "assistant",
        "content": [{"type": "text", "text": "Done."}], "usage": usage}}),
    ));
    assert_eq!(text[0].kind, EntryKind::Assistant { text: "Done.".into() });
    assert_eq!(text[0].message_id.as_deref(), Some("msg_1"));
    assert_eq!(
        text[0].usage.as_ref().map(|u| (u.input_tokens, u.output_tokens, u.cache_read_input_tokens)),
        Some((12, 340, 5000))
    );
    let tool = parse_line(&line(
        json!({"type": "assistant", "timestamp": "t2", "message": {"id": "msg_1", "role": "assistant",
        "content": [{"type": "tool_use", "name": "Bash", "input": {"command": "cargo test", "description": "run tests"}}]}}),
    ));
    assert_eq!(tool[0].kind, EntryKind::ToolUse { name: "Bash".into(), input: "cargo test".into() });
}

#[test]
fn tool_results_keep_their_output_and_error_flag() {
    let e = parse_line(&line(json!({"type": "user", "timestamp": "t3", "message": {"role": "user", "content": [
        {"type": "tool_result", "is_error": true, "content": [{"type": "text", "text": "error[E0425]"}]}]}})));
    assert_eq!(e[0].kind, EntryKind::ToolResult { is_error: true, output: "error[E0425]".into() });
}

#[test]
fn huge_outputs_are_clipped_and_other_lines_skipped() {
    let big = "x".repeat(100_000);
    let e = parse_line(&line(json!({"type": "user", "timestamp": "t", "message": {"role": "user", "content": [
        {"type": "tool_result", "content": big}]}})));
    match &e[0].kind {
        EntryKind::ToolResult { output, .. } => assert!(output.len() < 17_000, "{}", output.len()),
        other => panic!("{other:?}"),
    }
    for skipped in [
        json!({"type": "ai-title", "aiTitle": "x"}),
        json!({"type": "assistant", "message": {"content": [{"type": "thinking", "thinking": "…"}]}}),
    ] {
        assert!(parse_line(&line(skipped)).is_empty());
    }
    assert!(parse_line("not json").is_empty());
}
