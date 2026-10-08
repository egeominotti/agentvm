//! The guest's answer to a save request (`save.done`).

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveReply {
    Saved { commits: u32 },
    Failed(String),
}

#[derive(Deserialize)]
struct Raw {
    commits: u32,
    #[serde(default)]
    error: Option<String>,
}

impl SaveReply {
    /// `None` until the file holds complete JSON: the guest may still be writing it.
    pub fn parse(bytes: &[u8]) -> Option<SaveReply> {
        let raw: Raw = serde_json::from_slice(bytes).ok()?;
        Some(match raw.error {
            Some(e) => SaveReply::Failed(e),
            None => SaveReply::Saved { commits: raw.commits },
        })
    }
}
