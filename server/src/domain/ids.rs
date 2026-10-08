//! Identifiers validated at construction.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdError {
    #[error("invalid commit SHA: {0}")]
    InvalidSha(String),
    #[error("the prompt is empty")]
    EmptyPrompt,
    #[error("{0} is not a git repository")]
    NotARepo(String),
    #[error("invalid task id: {0}")]
    InvalidId(String),
}

/// A UUIDv7 (RFC 9562): the creation time in ms, then 74 random bits. Unique, and sorted by
/// time like the `YYYYMMDD-HHMMSS-xxxx` ids before it, which are still read.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TaskId(String);

impl TryFrom<String> for TaskId {
    type Error = IdError;
    fn try_from(s: String) -> Result<Self, IdError> {
        TaskId::parse(&s).ok_or(IdError::InvalidId(s))
    }
}

impl From<TaskId> for String {
    fn from(id: TaskId) -> String {
        id.0
    }
}

impl TaskId {
    /// `rand`: random bytes, 10 are used (fewer are padded with zeros, for tests).
    pub fn generate(now: SystemTime, rand: &[u8]) -> Self {
        TaskId(uuid_v7(now, rand))
    }

    /// Rebuilds an id received from outside (a URL, a file): a UUIDv7 in lower case, or the
    /// `YYYYMMDD-HHMMSS-xxxx` of tasks made before UUIDs.
    pub fn parse(s: &str) -> Option<Self> {
        (is_uuid_v7(s) || is_legacy(s)).then(|| TaskId(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn branch(&self) -> String {
        format!("agent/{}", self.0)
    }
}

/// RFC 9562 layout: 48-bit ms timestamp, version 7, 12 random bits, variant 10, 62 random bits.
fn uuid_v7(now: SystemTime, rand: &[u8]) -> String {
    let ms = now.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) & 0xffff_ffff_ffff;
    let mut r = [0u8; 10];
    for (slot, b) in r.iter_mut().zip(rand) {
        *slot = *b;
    }
    let mut b = [0u8; 16];
    b[..6].copy_from_slice(&ms.to_be_bytes()[2..]);
    b[6..].copy_from_slice(&r);
    b[6] = 0x70 | (b[6] & 0x0f);
    b[8] = 0x80 | (b[8] & 0x3f);
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

fn is_uuid_v7(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
        && b[14] == b'7'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b')
}

/// `YYYYMMDD-HHMMSS-xxxx`: tasks and snapshots made before UUIDs.
fn is_legacy(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b[8] == b'-'
        && b[15] == b'-'
        && b[..8].iter().chain(&b[9..15]).all(u8::is_ascii_digit)
        && b[16..].iter().all(|c| c.is_ascii_hexdigit())
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommitSha(String);

impl CommitSha {
    pub fn parse(s: &str) -> Result<Self, IdError> {
        let s = s.trim();
        if s.len() == 40 && s.bytes().all(|c| c.is_ascii_hexdigit()) {
            Ok(CommitSha(s.to_ascii_lowercase()))
        } else {
            Err(IdError::InvalidSha(s.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoPath(PathBuf);

impl RepoPath {
    pub fn new(path: PathBuf) -> Result<Self, IdError> {
        if path.join(".git").exists() { Ok(RepoPath(path)) } else { Err(IdError::NotARepo(path.display().to_string())) }
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Prompt(String);

impl Prompt {
    pub fn new(s: String) -> Result<Self, IdError> {
        let t = s.trim();
        if t.is_empty() { Err(IdError::EmptyPrompt) } else { Ok(Prompt(t.to_owned())) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
