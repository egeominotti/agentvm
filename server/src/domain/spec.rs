//! `task.json`: ciò che il guest riceve per eseguire un task.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    pub prompt: String,
    pub branch: String,
    pub base_sha: String,
    pub timeout_s: u64,
    /// Terminale interattivo: la VM resta accesa finché l'utente non la chiude.
    #[serde(default)]
    pub interactive: bool,
}
