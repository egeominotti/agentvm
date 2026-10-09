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

/// The result carries the run's tokens: an automatic task's usage counts them, not only its cost.
#[test]
fn the_result_carries_the_tokens_of_the_run() {
    let line = include_str!("../fixtures/stream-hello.jsonl").lines().last().unwrap();
    let Some(AgentEvent::Result { input_tokens, output_tokens, .. }) = parse_line(line).pop() else {
        panic!("no result in {line}")
    };
    let usage: serde_json::Value = serde_json::from_str::<serde_json::Value>(line).unwrap()["usage"].clone();
    let input = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
        .iter()
        .map(|k| usage[k].as_u64().unwrap_or(0))
        .sum::<u64>();
    assert!(input > 0 && output_tokens > 0, "{usage}");
    assert_eq!(input_tokens, input);
    assert_eq!(output_tokens, usage["output_tokens"].as_u64().unwrap());
}
