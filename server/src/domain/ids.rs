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

/// `YYYYMMDD-HHMMSS-xxxx` (UTC + 4 random hex digits): unique and sortable.
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
    pub fn generate(now: SystemTime, rand: [u8; 2]) -> Self {
        let secs = now.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let (y, m, d) = civil_from_days((secs / 86_400) as i64);
        let tod = secs % 86_400;
        TaskId(format!(
            "{y:04}{m:02}{d:02}-{:02}{:02}{:02}-{:02x}{:02x}",
            tod / 3600,
            tod / 60 % 60,
            tod % 60,
            rand[0],
            rand[1]
        ))
    }

    /// Rebuilds an id received from outside (e.g. from a URL); accepts only the generated format.
    pub fn parse(s: &str) -> Option<Self> {
        let b = s.as_bytes();
        let shape = b.len() == 20
            && b[8] == b'-'
            && b[15] == b'-'
            && b[..8].iter().chain(&b[9..15]).all(u8::is_ascii_digit)
            && b[16..].iter().all(|c| c.is_ascii_hexdigit());
        shape.then(|| TaskId(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn branch(&self) -> String {
        format!("agent/{}", self.0)
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Days since the epoch → (year, month, day), H. Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
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
