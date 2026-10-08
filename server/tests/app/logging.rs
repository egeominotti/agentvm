//! What the server logs about a VM: every state it goes through, and why it failed.

use agentvm::adapters::server_log::lines_for;
use agentvm::app::store::Store;
use agentvm::domain::task::TaskEvent;

use crate::helpers::record;

/// The only test of this binary that sets up logging (a process has one log).
#[test]
fn a_vms_states_and_its_failure_are_in_the_log() {
    let logs = tempfile::tempdir().unwrap();
    let guard = agentvm::logging::init(logs.path(), "info").expect("logging set up");
    let repo = tempfile::tempdir().unwrap();
    let store = Store::new();
    let rec = record(&repo);
    let id = rec.id.clone();
    store.insert(rec);
    store.apply(&id, TaskEvent::SlotAcquired).unwrap();
    store.apply(&id, TaskEvent::Failure("disk clone: No space left on device".into())).unwrap();
    drop(guard);
    let lines = lines_for(logs.path(), id.as_str(), 50).join("\n");
    assert!(lines.contains("\"to\":\"preparing\""), "{lines}");
    assert!(lines.contains("\"to\":\"failed\"") && lines.contains("No space left on device"), "{lines}");
    assert!(lines.contains("\"level\":\"WARN\""), "a failure is a warning: {lines}");
}
