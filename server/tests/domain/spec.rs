//! The task spec handed to the guest.

use agentvm::domain::spec::TaskSpec;

#[test]
fn task_spec_roundtrips_hostile_prompt() {
    let spec = TaskSpec {
        id: "20261008-154501-a3f9".into(),
        prompt: "it's $(rm -rf /)\nok \"quoted\"".into(),
        branch: "agent/20261008-154501-a3f9".into(),
        base_sha: "a".repeat(40),
        timeout_s: 1800,
        interactive: false,
        model: None,
        claude_version: None,
        restore: false,
    };
    let json = serde_json::to_string(&spec).unwrap();
    let back: TaskSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(back, spec);
}

#[test]
fn task_spec_interactive_defaults_to_false() {
    let spec: TaskSpec = serde_json::from_str(
        r#"{"id":"x","prompt":"p","branch":"agent/x","base_sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","timeout_s":1}"#,
    )
    .unwrap();
    assert!(!spec.interactive);
}
