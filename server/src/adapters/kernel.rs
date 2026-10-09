//! The kernel kept beside the golden image (`golden/boot/`, written by build-golden.sh): the files
//! a VM boots straight into, its command line, and the stamp naming the disk it was built with.

use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Where a job keeps its copy of the kernel, beside its disk (deleted with it).
const JOB_DIR: &str = "boot";

pub struct GoldenKernel {
    dir: PathBuf,
}

/// The kernel and initial ramdisk a VM boots from, in its own job folder.
#[derive(Debug, Clone)]
pub struct KernelFiles {
    pub kernel: PathBuf,
    pub initrd: PathBuf,
}

impl GoldenKernel {
    pub fn new(dir: PathBuf) -> Self {
        GoldenKernel { dir }
    }

    /// The identity of the disk the kernel was built with, as `disk_identity` gives it.
    pub fn stamp(&self) -> Option<String> {
        self.small("disk-id")
    }

    pub fn cmdline(&self) -> Option<String> {
        self.small("cmdline")
    }

    /// Copies the kernel and its initrd into `job` (clones on APFS: instant, no space taken). The
    /// VM boots from files of its own, which a rebuild of the image cannot replace under it.
    pub fn copy_into(&self, job: &Path) -> io::Result<KernelFiles> {
        let into = job.join(JOB_DIR);
        fs::create_dir_all(&into)?;
        let files = KernelFiles { kernel: into.join("vmlinuz"), initrd: into.join("initrd.img") };
        fs::copy(self.dir.join("vmlinuz"), &files.kernel)?;
        fs::copy(self.dir.join("initrd.img"), &files.initrd)?;
        Ok(files)
    }

    /// One short line written by the image's build; a file grown past that is not one of them.
    fn small(&self, name: &str) -> Option<String> {
        let mut text = String::new();
        fs::File::open(self.dir.join(name)).ok()?.take(4096).read_to_string(&mut text).ok()?;
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }
}

/// A disk image's identity: its inode and last write (seconds), through links, as
/// `stat -L -f '%i %m'` prints it. A rebuilt image is a new file, written by its own build.
pub fn disk_identity(path: &Path) -> io::Result<String> {
    let meta = fs::metadata(path)?;
    Ok(format!("{} {}", meta.ino(), meta.mtime()))
}
