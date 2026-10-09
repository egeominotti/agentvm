//! Snapshot disks as compressed, deduplicated chunks: `<root>/<ab>/<abcd…>.zst`, each named by
//! the BLAKE3 hash of its content and compressed with zstd at level 19. A chunk shared by many
//! snapshots (the system, files that did not change) is stored once. Packing and restoring run
//! on every core.

use std::collections::HashMap;
use std::ffi::{c_int, c_long};
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::domain::chunks::{CHUNK, ChunkId, Manifest};

/// The smallest files, paid for in CPU only for chunks never seen before (after a VM's first
/// snapshot, a few hundred MB at most).
const LEVEL: i32 = 19;
/// Runs of zeros at least this long are left as holes when a disk is restored.
const SPARSE_GRAIN: usize = 64 << 10;

#[derive(Clone)]
pub struct ChunkStore {
    root: PathBuf,
}

impl ChunkStore {
    pub fn new(root: PathBuf) -> Self {
        ChunkStore { root }
    }

    fn path(&self, id: &ChunkId) -> PathBuf {
        let id = id.as_str();
        self.root.join(&id[..2]).join(format!("{id}.zst"))
    }

    /// Stores every non-empty chunk of `disk` not already here; the disk as a manifest.
    pub fn pack(&self, disk: &Path) -> io::Result<Manifest> {
        let file = File::open(disk)?;
        let size = file.metadata()?.len();
        let indices = data_chunks(&file, size)?;
        let found = Mutex::new(Vec::with_capacity(indices.len()));
        parallel(indices.len(), |i| {
            let index = indices[i];
            let offset = index * CHUNK;
            let mut buf = vec![0u8; CHUNK.min(size - offset) as usize];
            file.read_exact_at(&mut buf, offset)?;
            if buf.iter().all(|b| *b == 0) {
                return Ok(());
            }
            let id = ChunkId::parse(blake3::hash(&buf).to_hex().as_str()).expect("BLAKE3 hex");
            self.store(&id, &buf)?;
            found.lock().unwrap_or_else(|e| e.into_inner()).push((index, id));
            Ok(())
        })?;
        let mut chunks = found.into_inner().unwrap_or_else(|e| e.into_inner());
        chunks.sort_by_key(|(i, _)| *i);
        Ok(Manifest { size, chunk: CHUNK, chunks })
    }

    /// Writes the disk of `m` to `dest` (a new file, sparse where the disk is empty). Every chunk
    /// is checked against its hash: a damaged one fails the restore and leaves nothing behind.
    pub fn unpack(&self, m: &Manifest, dest: &Path) -> io::Result<()> {
        if m.chunk != CHUNK {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "unknown chunk size"));
        }
        let file = fs::OpenOptions::new().write(true).create_new(true).open(dest)?;
        let result = (|| {
            file.set_len(m.size)?;
            parallel(m.chunks.len(), |i| {
                let (index, id) = &m.chunks[i];
                let data = zstd::bulk::decompress(&fs::read(self.path(id))?, CHUNK as usize)?;
                if blake3::hash(&data).to_hex().as_str() != id.as_str() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("chunk {} is damaged", id.as_str()),
                    ));
                }
                // Only the parts holding data: the restored disk stays as sparse as the original.
                for (n, part) in data.chunks(SPARSE_GRAIN).enumerate() {
                    if part.iter().any(|b| *b != 0) {
                        file.write_all_at(part, index * CHUNK + (n * SPARSE_GRAIN) as u64)?;
                    }
                }
                Ok(())
            })?;
            file.sync_all()
        })();
        if result.is_err() {
            let _ = fs::remove_file(dest);
        }
        result
    }

    /// Every chunk stored, with its size on disk.
    pub fn present(&self) -> io::Result<HashMap<ChunkId, u64>> {
        let mut out = HashMap::new();
        let Ok(dirs) = fs::read_dir(&self.root) else { return Ok(out) };
        for dir in dirs.flatten() {
            for f in fs::read_dir(dir.path())?.flatten() {
                let name = f.file_name().to_string_lossy().into_owned();
                if let Some(id) = name.strip_suffix(".zst").and_then(ChunkId::parse) {
                    out.insert(id, f.metadata()?.len());
                }
            }
        }
        Ok(out)
    }

    /// Deletes `ids`; the bytes freed.
    pub fn remove(&self, ids: &[ChunkId]) -> io::Result<u64> {
        let mut freed = 0;
        for id in ids {
            let p = self.path(id);
            if let Ok(meta) = fs::metadata(&p) {
                fs::remove_file(&p)?;
                freed += meta.len();
            }
        }
        Ok(freed)
    }

    /// The file of chunk `id` (to hard-link it into an export).
    pub fn file(&self, id: &ChunkId) -> PathBuf {
        self.path(id)
    }

    /// Takes in a chunk file from an import, if this store lacks it (its content is checked
    /// against its name when it is restored).
    pub fn adopt(&self, id: &ChunkId, file: &Path) -> io::Result<()> {
        let dest = self.path(id);
        if dest.exists() {
            return Ok(());
        }
        fs::create_dir_all(dest.parent().expect("chunk in a folder"))?;
        fs::rename(file, dest)
    }

    /// Compressed and written under a temporary name, then renamed: a chunk file is whole or absent.
    fn store(&self, id: &ChunkId, data: &[u8]) -> io::Result<()> {
        let dest = self.path(id);
        if dest.exists() {
            return Ok(());
        }
        let dir = dest.parent().expect("chunk in a folder");
        fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".{}.{:?}", id.as_str(), std::thread::current().id()));
        fs::write(&tmp, zstd::bulk::compress(data, LEVEL)?)?;
        fs::rename(&tmp, &dest)
    }
}

/// Runs `job(0..n)` on every core; the first error stops the others.
fn parallel(n: usize, job: impl Fn(usize) -> io::Result<()> + Sync) -> io::Result<()> {
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let error = Mutex::new(None);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(n.max(1));
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    if let Err(e) = job(i) {
                        stop.store(true, Ordering::Relaxed);
                        error.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(e);
                    }
                }
            });
        }
    });
    error.into_inner().unwrap_or_else(|e| e.into_inner()).map_or(Ok(()), Err)
}

const SEEK_HOLE: c_int = 3;
const SEEK_DATA: c_int = 4;

unsafe extern "C" {
    fn lseek(fd: c_int, offset: c_long, whence: c_int) -> c_long;
}

/// The chunks holding data: only those are read (a 20 GB disk holds 3 GB at most). A filesystem
/// that cannot say where data is gets every chunk read.
fn data_chunks(file: &File, size: u64) -> io::Result<Vec<u64>> {
    let fd = file.as_raw_fd();
    let mut out: Vec<u64> = Vec::new();
    let mut pos: u64 = 0;
    while pos < size {
        // SAFETY: a valid descriptor; lseek only moves the offset of this read-only file.
        let data = unsafe { lseek(fd, pos as c_long, SEEK_DATA) };
        if data < 0 {
            let e = io::Error::last_os_error();
            return match e.raw_os_error() {
                Some(6) => Ok(out),                                              // ENXIO: no data after `pos`
                Some(22) if pos == 0 => Ok((0..size.div_ceil(CHUNK)).collect()), // EINVAL: no support
                _ => Err(e),
            };
        }
        // SAFETY: as above.
        let hole = unsafe { lseek(fd, data, SEEK_HOLE) };
        let end = if hole < 0 { size } else { hole as u64 };
        let first = data as u64 / CHUNK;
        let last = end.div_ceil(CHUNK);
        for i in first..last {
            if out.last() != Some(&i) {
                out.push(i);
            }
        }
        pos = last * CHUNK;
    }
    Ok(out)
}
