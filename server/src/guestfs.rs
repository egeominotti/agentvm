//! Files in a job's shared folder are written by the guest, which is root in its VM: any of them
//! may be a symlink to a file of this Mac, a FIFO, or huge. Every host access goes through here.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

// macOS <fcntl.h>.
const O_NONBLOCK: i32 = 0x0004;
const O_NOFOLLOW: i32 = 0x0100;

/// Opens `path` for reading only if it is a regular file: symlinks are refused by the kernel and
/// FIFOs neither block (O_NONBLOCK) nor pass the type check.
pub fn open_regular(path: &Path) -> Option<File> {
    let f = OpenOptions::new().read(true).custom_flags(O_NOFOLLOW | O_NONBLOCK).open(path).ok()?;
    f.metadata().ok()?.is_file().then_some(f)
}

/// The whole file, or `None` if it is larger than `max` bytes or not a regular file.
pub fn read(path: &Path, max: u64) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    open_regular(path)?.take(max + 1).read_to_end(&mut buf).ok()?;
    (buf.len() as u64 <= max).then_some(buf)
}

/// At most the first `max` bytes of a regular file.
pub fn read_prefix(path: &Path, max: u64) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    open_regular(path)?.take(max).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Creates an empty `path` for the guest to see. Whatever the guest left there is removed first
/// (a symlink goes away itself, its target is untouched), and the new file is never followed.
pub fn create_empty(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    OpenOptions::new().write(true).create_new(true).custom_flags(O_NOFOLLOW).open(path).map(drop)
}

/// Copies a regular file the guest produced to a place only the host controls.
pub fn copy_out(from: &Path, to: &Path) -> io::Result<u64> {
    let mut src =
        open_regular(from).ok_or_else(|| io::Error::other(format!("{} is not a regular file", from.display())))?;
    let mut dst = File::create(to)?;
    io::copy(&mut src, &mut dst)
}
