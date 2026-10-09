//! The kernel kept beside the golden image: its stamp, its command line, its copy into a job.

use std::fs;
use std::path::Path;

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::adapters::kernel::{GoldenKernel, disk_identity};

use crate::helpers::task_id;

/// What build-golden.sh writes beside the image, as the shell sees it.
fn stat_identity(path: &Path) -> String {
    let out = std::process::Command::new("stat").args(["-L", "-f", "%i %m"]).arg(path).output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn golden_kernel(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("vmlinuz"), "kernel image").unwrap();
    fs::write(dir.join("initrd.img"), "initial ramdisk").unwrap();
    fs::write(dir.join("cmdline"), "root=PARTUUID=abc ro console=hvc0\n").unwrap();
    fs::write(dir.join("disk-id"), "1234 1700000000\n").unwrap();
}

#[test]
fn the_disk_identity_is_the_one_build_golden_writes_and_follows_links() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = tmp.path().join("disk.raw");
    fs::write(&disk, "disk").unwrap();
    // The image's last write (the build's VM) is not when the file was born: clonefile carries
    // the birth time of Debian's download over to every build.
    let written = std::time::SystemTime::now() + std::time::Duration::from_secs(86_400);
    fs::File::options().write(true).open(&disk).unwrap().set_modified(written).unwrap();
    let link = tmp.path().join("link.raw");
    std::os::unix::fs::symlink(&disk, &link).unwrap();
    assert_eq!(disk_identity(&disk).unwrap(), stat_identity(&disk));
    // Test and sandbox homes link the shared image: the link names the same disk.
    assert_eq!(disk_identity(&link).unwrap(), disk_identity(&disk).unwrap());
}

#[test]
fn a_rebuilt_image_is_another_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = tmp.path().join("disk.raw");
    fs::write(&disk, "first build").unwrap();
    let first = disk_identity(&disk).unwrap();
    // As build-golden.sh does: the new image is moved over the old one.
    let next = tmp.path().join("next.raw");
    fs::write(&next, "second build").unwrap();
    fs::rename(&next, &disk).unwrap();
    assert_ne!(disk_identity(&disk).unwrap(), first);
}

#[test]
fn a_job_gets_its_own_copy_of_the_kernel_and_loses_it_with_its_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let boot = tmp.path().join("golden/boot");
    golden_kernel(&boot);
    let golden = GoldenKernel::new(boot.clone());
    assert_eq!(golden.stamp().as_deref(), Some("1234 1700000000"));
    assert_eq!(golden.cmdline().as_deref(), Some("root=PARTUUID=abc ro console=hvc0"));

    let ws = JobWorkspace::create(&tmp.path().join("jobs"), &task_id()).unwrap();
    let files = golden.copy_into(ws.dir()).unwrap();
    // A rebuild of the image replaces its kernel: the VM keeps booting from its own copy.
    fs::remove_dir_all(&boot).unwrap();
    assert_eq!(fs::read_to_string(&files.kernel).unwrap(), "kernel image");
    assert_eq!(fs::read_to_string(&files.initrd).unwrap(), "initial ramdisk");
    assert!(files.kernel.starts_with(ws.dir()) && files.initrd.starts_with(ws.dir()));

    drop(ws);
    assert!(!files.kernel.exists() && !files.initrd.exists());
}

#[test]
fn an_image_without_a_kernel_beside_it_has_nothing_to_offer() {
    let tmp = tempfile::tempdir().unwrap();
    let golden = GoldenKernel::new(tmp.path().join("golden/boot"));
    assert_eq!(golden.stamp(), None);
    assert_eq!(golden.cmdline(), None);
    let ws = JobWorkspace::create(&tmp.path().join("jobs"), &task_id()).unwrap();
    assert!(golden.copy_into(ws.dir()).is_err());
}
