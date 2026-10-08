//! The guest's reply to a save request.

use agentvm::domain::save::SaveReply;

#[test]
fn save_reply_waits_for_complete_json_and_reports_guest_errors() {
    // The guest may be halfway through writing the file when the host looks.
    assert_eq!(SaveReply::parse(b""), None);
    assert_eq!(SaveReply::parse(b"{\"comm"), None);
    assert_eq!(SaveReply::parse(br#"{"commits":2}"#), Some(SaveReply::Saved { commits: 2 }));
    assert_eq!(
        SaveReply::parse(br#"{"commits":0,"error":"git commit failed: index.lock exists"}"#),
        Some(SaveReply::Failed("git commit failed: index.lock exists".into()))
    );
}
