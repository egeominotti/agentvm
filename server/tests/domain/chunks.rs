//! Snapshot disks as lists of compressed 1 MiB chunks shared by every snapshot: which chunks are
//! garbage, and what each snapshot really costs.

use std::collections::HashMap;

use agentvm::domain::chunks::{ChunkId, Manifest, own_bytes, unreferenced};

fn id(n: u8) -> ChunkId {
    ChunkId::parse(&format!("{n:02x}").repeat(32)).unwrap()
}

fn manifest(ids: &[u8]) -> Manifest {
    Manifest {
        size: 20 << 30,
        chunk: 1 << 20,
        chunks: ids.iter().enumerate().map(|(i, n)| (i as u64, id(*n))).collect(),
    }
}

#[test]
fn chunk_ids_are_blake3_hex_and_nothing_else() {
    assert!(ChunkId::parse(&"ab".repeat(32)).is_some());
    for bad in ["", "ab", &"AB".repeat(32), &"zz".repeat(32), &"ab".repeat(33), "../../../../etc/passwd"] {
        assert!(ChunkId::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn only_chunks_no_snapshot_uses_are_garbage() {
    let kept = [manifest(&[1, 2, 3]), manifest(&[3, 4])];
    let mut gone = unreferenced([id(1), id(2), id(3), id(4), id(5), id(6)], &kept);
    gone.sort();
    assert_eq!(gone, [id(5), id(6)]);
    assert!(unreferenced([id(1)], &[manifest(&[1])]).is_empty());
}

/// What deleting a snapshot frees: the chunks only it uses. Shared ones (the system, files that
/// did not change) count for none of them.
#[test]
fn a_snapshot_costs_the_chunks_only_it_uses() {
    let sizes: HashMap<ChunkId, u64> = [(id(1), 900), (id(2), 50), (id(3), 7), (id(4), 30)].into_iter().collect();
    let first = manifest(&[1, 2]);
    let second = manifest(&[1, 3]);
    let third = manifest(&[1, 3, 4, 4]);
    let own = own_bytes(&[("a", &first), ("b", &second), ("c", &third)], &sizes);
    assert_eq!(own["a"], 50);
    assert_eq!(own["b"], 0, "chunk 3 is shared with c");
    assert_eq!(own["c"], 30, "a chunk used twice by one snapshot counts once");
}
