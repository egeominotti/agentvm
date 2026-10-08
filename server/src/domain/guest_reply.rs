//! Replies from a VM to the host's requests: a request carries an id, its reply echoes it, so a
//! late reply to an earlier request is never taken for the current one.

use serde_json::Value;

/// The reply in `bytes` if it answers request `id`; `None` while it does not (yet): JSON still
/// being written, or the answer to another request. A reply without an id comes from a guest
/// started before requests had ids (an empty file, for a flush) and is accepted as it is.
pub fn answer_to(bytes: &[u8], id: &str) -> Option<Value> {
    if bytes.is_empty() {
        return Some(Value::Null);
    }
    let reply: Value = serde_json::from_slice(bytes).ok()?;
    match reply.get("id").and_then(Value::as_str) {
        Some(other) if other != id => None,
        _ => Some(reply),
    }
}
