//! Booting straight into the kernel kept beside the golden image, without the firmware and GRUB.

use agentvm::adapters::kernel::{GoldenKernel, disk_identity};
use agentvm::adapters::vm::{DirectBoot, VmEvent, VmProcess};
use agentvm::domain::boot::{boots_directly, kernel_cmdline};
use agentvm::domain::spec::TaskSpec;
use agentvm::secret::Secret;

use crate::helpers::{KillVms, config, golden, helper, repo_with_bundle, shell, workspace};

#[tokio::test]
#[ignore = "requires the golden image built with its kernel beside it (scripts/build-golden.sh)"]
async fn a_clone_boots_straight_into_the_kernel_built_with_its_image() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    let image = golden();
    let kernel = GoldenKernel::new(image.with_file_name("boot"));
    let disk = disk_identity(&image).unwrap();
    assert!(boots_directly(kernel.stamp().as_deref(), &disk, &disk), "no kernel built with this image beside it");
    let files = kernel.copy_into(ws.dir()).unwrap();
    let cmdline = kernel_cmdline(&kernel.cmdline().unwrap()).unwrap();

    let base = repo_with_bundle(&ws);
    ws.write_spec(&TaskSpec {
        id: "t".into(),
        prompt: String::new(),
        branch: "agent/t".into(),
        base_sha: base,
        timeout_s: 60,
        interactive: true,
        model: None,
        claude_version: None,
        restore: false,
    })
    .unwrap();
    ws.write_token(&Secret::new("sk-ant-oat01-not-a-real-token".into())).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    cfg.direct = Some(DirectBoot { kernel: files.kernel, initrd: files.initrd, cmdline });
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.dir().join("vm.events")).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));

    // A shell answers only once the job runs: the shared folder (virtiofs) and the terminals
    // (vsock) work, so the kernel found its own modules on the disk.
    let out = shell(&ws.pty_socket(), "cat /proc/cmdline").await;
    // GRUB names the file it booted; the host's kernel has no such word.
    assert!(out.contains("root=PARTUUID=") && !out.contains("BOOT_IMAGE="), "{out}");
    // The EFI variables are there all the same: a snapshot of this VM boots through EFI.
    assert!(ws.efivars().is_file());
    vm.terminate();
    vm.wait().await;
}
