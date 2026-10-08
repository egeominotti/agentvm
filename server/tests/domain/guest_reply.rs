//! Matching a VM's reply to the request it answers.

use agentvm::domain::guest_reply::answer_to;
use serde_json::{Value, json};

#[test]
fn a_reply_answers_only_the_request_it_names() {
    let mine = br#"{"id":"r2","commits":3}"#;
    assert_eq!(answer_to(mine, "r2"), Some(json!({"id": "r2", "commits": 3})));
    assert_eq!(answer_to(br#"{"id":"r1","commits":9}"#, "r2"), None, "a late reply to r1");
}

#[test]
fn half_written_json_is_not_an_answer_yet() {
    assert_eq!(answer_to(br#"{"id":"r2","comm"#, "r2"), None);
}

/// VMs started before requests had ids keep working: their replies are taken as they are.
#[test]
fn replies_from_older_guests_are_accepted() {
    assert_eq!(answer_to(b"", "r2"), Some(Value::Null));
    assert_eq!(answer_to(br#"{"commits":1}"#, "r2"), Some(json!({"commits": 1})));
}
