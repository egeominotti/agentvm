//! Agent events (real recorded stream).

use agentvm::domain::agent_event::{AgentEvent, parse_line};

#[test]
fn parses_real_stream() {
    let events: Vec<AgentEvent> = include_str!("../fixtures/stream-hello.jsonl").lines().flat_map(parse_line).collect();
    assert!(
        matches!(&events[0], AgentEvent::Init { model, claude_code_version } if model == "claude-sonnet-5-5" && claude_code_version == "2.1.294")
    );
    assert!(events.iter().any(
        |e| matches!(e, AgentEvent::ToolUse { name, summary } if name == "Bash" && summary.contains("hello.txt"))
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolResult { is_error: false, summary } if summary.contains("aarch64")))
    );
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Text { text } if text.contains("hello.txt"))));
    assert!(matches!(events.last().unwrap(), AgentEvent::Result { is_error: false, duration_ms: 6534, .. }));
    assert!(!events.iter().any(|e| matches!(e, AgentEvent::Unparsed { .. })));
}

#[test]
fn truncated_line_is_unparsed() {
    let ev = parse_line("{\"type\":\"assi");
    assert!(matches!(ev.as_slice(), [AgentEvent::Unparsed { raw }] if raw == "{\"type\":\"assi"));
}

#[test]
fn retry_event() {
    let ev = parse_line(r#"{"type":"system","subtype":"api_retry","attempt":3,"max_retries":10}"#);
    assert!(matches!(ev.as_slice(), [AgentEvent::Retry { attempt: 3 }]));
}

#[test]
fn long_summaries_are_truncated() {
    let long = "x".repeat(1000);
    let line = serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command": long}}]}}).to_string();
    match parse_line(&line).as_slice() {
        [AgentEvent::ToolUse { summary, .. }] => assert!(summary.chars().count() <= 301),
        other => panic!("{other:?}"),
    }
}

#[test]
fn agent_event_serializes_with_kind() {
    let v = serde_json::to_value(AgentEvent::Retry { attempt: 1 }).unwrap();
    assert_eq!(v, serde_json::json!({"kind": "retry", "attempt": 1}));
}

/// A tool that prints megabytes (a log, a minified file) becomes one bounded event: the server
/// keeps events in memory for every task.
#[test]
fn huge_texts_are_clipped() {
    use agentvm::domain::agent_event::{AgentEvent, MAX_TEXT, parse_line};
    let big = "x".repeat(5 * MAX_TEXT);
    let line = serde_json::json!({"type": "assistant", "message": {"content": [{"type": "text", "text": big}]}});
    match &parse_line(&line.to_string())[..] {
        [AgentEvent::Text { text }] => assert!(text.len() <= MAX_TEXT + 64 && text.ends_with('…'), "{}", text.len()),
        other => panic!("{other:?}"),
    }
    match &parse_line(&"y".repeat(5 * MAX_TEXT))[..] {
        [AgentEvent::Unparsed { raw }] => assert!(raw.len() <= MAX_TEXT + 64),
        other => panic!("{other:?}"),
    }
}
