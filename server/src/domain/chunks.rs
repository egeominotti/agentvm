//! Snapshot disks stored as compressed chunks shared by every snapshot: a disk is a `Manifest`,
//! the list of its non-empty 1 MiB chunks; equal chunks (the system, files that did not change)
//! are stored once. What is pure about it: which chunks are garbage, what a snapshot costs.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Chunk size: VM disks change in filesystem blocks, and 1 MiB keeps both the number of chunks
/// and the bytes rewritten for a small change low.
pub const CHUNK: u64 = 1 << 20;

/// A chunk's BLAKE3 hash, in lowercase hex: it names the chunk's file, so nothing else is accepted.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChunkId(String);

impl ChunkId {
    pub fn parse(s: &str) -> Option<Self> {
        (s.len() == 64 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
            .then(|| ChunkId(s.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ChunkId {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        ChunkId::parse(&s).ok_or_else(|| format!("not a chunk id: {s:?}"))
    }
}

impl From<ChunkId> for String {
    fn from(c: ChunkId) -> String {
        c.0
    }
}

/// A disk: its size and, for each chunk that is not all zeros, its index and content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub size: u64,
    pub chunk: u64,
    pub chunks: Vec<(u64, ChunkId)>,
}

impl Manifest {
    pub fn ids(&self) -> impl Iterator<Item = &ChunkId> {
        self.chunks.iter().map(|(_, id)| id)
    }
}

/// The chunks of `present` that no manifest uses (safe to delete).
pub fn unreferenced(present: impl IntoIterator<Item = ChunkId>, kept: &[Manifest]) -> Vec<ChunkId> {
    let used: HashSet<&ChunkId> = kept.iter().flat_map(Manifest::ids).collect();
    present.into_iter().filter(|c| !used.contains(c)).collect()
}

/// For each snapshot, the bytes deleting it frees: the chunks it alone uses (each counted once).
pub fn own_bytes<K: Copy + Eq + std::hash::Hash>(
    snapshots: &[(K, &Manifest)],
    sizes: &HashMap<ChunkId, u64>,
) -> HashMap<K, u64> {
    let mut users: HashMap<&ChunkId, HashSet<K>> = HashMap::new();
    for (key, m) in snapshots {
        for c in m.ids() {
            users.entry(c).or_default().insert(*key);
        }
    }
    let mut own: HashMap<K, u64> = snapshots.iter().map(|(k, _)| (*k, 0)).collect();
    for (c, who) in users {
        if who.len() == 1
            && let Some(k) = who.into_iter().next()
        {
            *own.entry(k).or_default() += sizes.get(c).copied().unwrap_or(0);
        }
    }
    own
}
