//! Files in a job's shared folder are written by the guest, which is root in its VM: any of them
//! may be a symlink to a file of this Mac, a FIFO, or huge. Every host access goes through here.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

// macOS <fcntl.h>.
const O_WRONLY: i32 = 0x0001;
const O_NONBLOCK: i32 = 0x0004;
const O_NOFOLLOW: i32 = 0x0100;
const O_CREAT: i32 = 0x0200;
const O_EXCL: i32 = 0x0800;
const O_DIRECTORY: i32 = 0x0010_0000;

unsafe extern "C" {
    fn openat(dirfd: std::ffi::c_int, path: *const std::ffi::c_char, flags: std::ffi::c_int, ...) -> std::ffi::c_int;
}

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

/// At most the last `max` bytes of a regular file.
pub fn read_suffix(path: &Path, max: u64) -> Option<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    let mut file = open_regular(path)?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max))).ok()?;
    let mut buf = Vec::new();
    file.take(max).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Creates an empty `path` for the guest to see (see `create_with`).
pub fn create_empty(path: &Path) -> io::Result<()> {
    create_with(path, b"")
}

/// Creates `path` holding `bytes` for the guest to see. Whatever the guest left there is removed
/// first (a symlink goes away itself, its target is untouched), and the new file is never followed.
pub fn create_with(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut file = OpenOptions::new().write(true).create_new(true).custom_flags(O_NOFOLLOW).open(path)?;
    std::io::Write::write_all(&mut file, bytes)
}

/// Puts `path` in place for the guest already holding `bytes`: written aside, then renamed (a
/// rename replaces whatever the guest left at `path`, a symlink included, without following it).
/// The guest sees either no file or the whole of it, never an empty one.
pub fn publish(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}-{n}.tmp", std::process::id()));
    let result = create_with(&tmp, bytes).and_then(|_| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Copies a regular file the guest produced to a place only the host controls.
pub fn copy_out(from: &Path, to: &Path) -> io::Result<u64> {
    let mut src =
        open_regular(from).ok_or_else(|| io::Error::other(format!("{} is not a regular file", from.display())))?;
    let mut dst = File::create(to)?;
    io::copy(&mut src, &mut dst)
}

/// Creates the new file `name` in `dir` (created if missing). `dir` is opened without following a
/// symlink and the file is created relative to that handle, so a guest that swaps `dir` for a
/// symlink cannot make the host write anywhere else; an existing file is never overwritten.
pub fn create_in(dir: &Path, name: &str) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\0']) || name.len() > 255 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("invalid file name {name:?}")));
    }
    match fs::create_dir(dir) {
        Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
        _ => {}
    }
    let dir = OpenOptions::new().read(true).custom_flags(O_NOFOLLOW | O_DIRECTORY).open(dir)?;
    let cname = std::ffi::CString::new(name)?;
    // SAFETY: valid directory descriptor and NUL-terminated name; mode is the variadic argument.
    let fd = unsafe { openat(dir.as_raw_fd(), cname.as_ptr(), O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0o644) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` was just opened and is owned by nobody else.
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// `create_in`, picking `notes (2).txt`, `notes (3).txt`… when `name` is taken.
pub fn create_unique(dir: &Path, name: &str) -> io::Result<(File, String)> {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    for n in 1..1000 {
        let candidate = if n == 1 { name.to_owned() } else { format!("{stem} ({n}){ext}") };
        match create_in(dir, &candidate) {
            Ok(f) => return Ok((f, candidate)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("too many files called {name}")))
}
