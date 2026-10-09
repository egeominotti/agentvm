//! Snapshot disks packed into compressed, deduplicated chunks and restored from them: byte for
//! byte, still sparse, and never from a damaged chunk.

use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::Path;

use agentvm::adapters::chunks::ChunkStore;

const MB: u64 = 1 << 20;

/// 64 MiB, mostly holes: two chunks with the same text, one with a little binary, one written
/// with zeros (allocated, but empty).
fn sparse_disk(path: &Path) {
    let f = std::fs::File::create(path).unwrap();
    f.set_len(64 * MB).unwrap();
    let line = b"fn main() { println!(\"hello\"); }\n";
    let text: Vec<u8> = (0..MB as usize).map(|i| line[i % line.len()]).collect();
    f.write_all_at(&text, 0).unwrap();
    f.write_all_at(&text, 10 * MB).unwrap();
    f.write_all_at(&[0xA5; 4096], 30 * MB + 512).unwrap();
    f.write_all_at(&vec![0u8; MB as usize], 40 * MB).unwrap();
}

fn files_under(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        if e.path().is_dir() {
            out.extend(files_under(&e.path()));
        } else {
            out.push(e.path());
        }
    }
    out
}

#[test]
fn a_disk_comes_back_byte_for_byte_and_still_sparse() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = tmp.path().join("disk.raw");
    sparse_disk(&disk);
    let store = ChunkStore::new(tmp.path().join("chunks"));
    let manifest = store.pack(&disk).unwrap();
    assert_eq!(manifest.size, 64 * MB);
    assert_eq!(manifest.chunks.iter().map(|(i, _)| *i).collect::<Vec<_>>(), [0, 10, 30], "zeros are not stored");
    assert_eq!(manifest.chunks[0].1, manifest.chunks[1].1, "equal chunks share one id");
    let stored = files_under(&tmp.path().join("chunks"));
    assert_eq!(stored.len(), 2, "{stored:?}");
    let bytes: u64 = stored.iter().map(|p| std::fs::metadata(p).unwrap().len()).sum();
    assert!(bytes < MB / 10, "the text chunk is compressed: {bytes} bytes");

    let back = tmp.path().join("restored.raw");
    store.unpack(&manifest, &back).unwrap();
    assert_eq!(std::fs::read(&back).unwrap(), std::fs::read(&disk).unwrap());
    let allocated = |p: &Path| std::fs::metadata(p).unwrap().blocks() * 512;
    assert!(
        allocated(&back) <= allocated(&disk),
        "the restored disk is as sparse as the original: {} vs {} bytes allocated",
        allocated(&back),
        allocated(&disk)
    );
}

#[test]
fn packing_the_same_disk_again_stores_nothing_new() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = tmp.path().join("disk.raw");
    sparse_disk(&disk);
    let store = ChunkStore::new(tmp.path().join("chunks"));
    let first = store.pack(&disk).unwrap();
    let before = files_under(&tmp.path().join("chunks"));
    assert_eq!(store.pack(&disk).unwrap(), first);
    assert_eq!(files_under(&tmp.path().join("chunks")), before);
    assert_eq!(store.present().unwrap().len(), 2);
}

#[test]
fn a_damaged_chunk_is_never_restored() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = tmp.path().join("disk.raw");
    sparse_disk(&disk);
    let store = ChunkStore::new(tmp.path().join("chunks"));
    let manifest = store.pack(&disk).unwrap();
    for f in files_under(&tmp.path().join("chunks")) {
        let mut b = std::fs::read(&f).unwrap();
        let mid = b.len() / 2;
        b[mid] ^= 0xFF;
        std::fs::write(&f, b).unwrap();
    }
    let back = tmp.path().join("restored.raw");
    assert!(store.unpack(&manifest, &back).is_err());
    assert!(!back.exists(), "no half-restored disk is left behind");
}

#[test]
fn chunks_no_snapshot_uses_are_removed() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = tmp.path().join("disk.raw");
    sparse_disk(&disk);
    let store = ChunkStore::new(tmp.path().join("chunks"));
    let manifest = store.pack(&disk).unwrap();
    let ids: Vec<_> = store.present().unwrap().into_keys().collect();
    let freed = store.remove(&ids).unwrap();
    assert!(freed > 0);
    assert!(store.present().unwrap().is_empty());
    assert!(store.unpack(&manifest, &tmp.path().join("gone.raw")).is_err());
}
