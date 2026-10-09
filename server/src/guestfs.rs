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
    fn unlinkat(dirfd: std::ffi::c_int, path: *const std::ffi::c_char, flags: std::ffi::c_int) -> std::ffi::c_int;
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
/// At most `max_bytes`: a guest's file larger than the room on the Mac's disk (a sparse file is
/// free in the guest, real bytes here) is refused before anything is written.
pub fn copy_out(from: &Path, to: &Path, max_bytes: u64) -> io::Result<u64> {
    let mut src =
        open_regular(from).ok_or_else(|| io::Error::other(format!("{} is not a regular file", from.display())))?;
    let len = src.metadata()?.len();
    if len > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            format!("{} is {} MB, more than the {} MB free on this Mac", from.display(), len >> 20, max_bytes >> 20),
        ));
    }
    let mut dst = File::create(to)?;
    io::copy(&mut src, &mut dst)
}

/// Creates the new file `name` in `dir` (created if missing). `dir` is opened without following a
/// symlink and the file is created relative to that handle, so a guest that swaps `dir` for a
/// symlink cannot make the host write anywhere else; an existing file is never overwritten.
pub fn create_in(dir: &Path, name: &str) -> io::Result<File> {
    create_at(dir, name).map(|created| created.file)
}

/// A file just created in a guest folder, with that folder's handle: removing it goes through
/// the handle, never through a path the guest may have turned into a link meanwhile.
pub struct NewFile {
    pub file: File,
    pub name: String,
    dir: File,
}

impl NewFile {
    /// Removes the file (a failed upload) from the folder it was created in.
    pub fn discard(self) {
        use std::os::fd::AsRawFd;
        let Ok(name) = std::ffi::CString::new(self.name) else { return };
        // SAFETY: valid directory descriptor and NUL-terminated name.
        unsafe { unlinkat(self.dir.as_raw_fd(), name.as_ptr(), 0) };
    }
}

fn create_at(dir: &Path, name: &str) -> io::Result<NewFile> {
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
    Ok(NewFile { file: unsafe { File::from_raw_fd(fd) }, name: name.to_owned(), dir })
}

/// `create_in`, picking `notes (2).txt`, `notes (3).txt`… when `name` is taken.
pub fn create_unique(dir: &Path, name: &str) -> io::Result<NewFile> {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    for n in 1..1000 {
        let candidate = if n == 1 { name.to_owned() } else { format!("{stem} ({n}){ext}") };
        match create_at(dir, &candidate) {
            Ok(created) => return Ok(created),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("too many files called {name}")))
}

/// A folder of the guest's, opened without following a symlink: its files are opened relative
/// to that handle, so swapping the folder (or a file in it) for a symlink never makes the host
/// read a file of this Mac.
pub struct GuestDir {
    path: std::path::PathBuf,
    fd: File,
}

impl GuestDir {
    pub fn open(path: &Path) -> Option<GuestDir> {
        let fd = OpenOptions::new().read(true).custom_flags(O_NOFOLLOW | O_DIRECTORY).open(path).ok()?;
        Some(GuestDir { path: path.to_path_buf(), fd })
    }

    /// The names of its entries (only names: contents are read through `open_file`).
    pub fn names(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.path) else { return Vec::new() };
        entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()
    }

    /// The regular file `name` of this folder, never through a symlink, never blocking on a FIFO.
    pub fn open_file(&self, name: &str) -> Option<File> {
        use std::os::fd::{AsRawFd, FromRawFd};
        if name.is_empty() || name.contains(['/', '\0']) || name == "." || name == ".." {
            return None;
        }
        let cname = std::ffi::CString::new(name).ok()?;
        // SAFETY: valid directory descriptor and NUL-terminated name.
        let fd = unsafe { openat(self.fd.as_raw_fd(), cname.as_ptr(), O_NOFOLLOW | O_NONBLOCK) };
        if fd < 0 {
            return None;
        }
        // SAFETY: `fd` was just opened and is owned by nobody else.
        let f = unsafe { File::from_raw_fd(fd) };
        f.metadata().ok()?.is_file().then_some(f)
    }
}
