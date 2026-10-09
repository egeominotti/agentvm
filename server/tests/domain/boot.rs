//! Whether a VM boots straight into the golden image's kernel, and with which command line.

use agentvm::domain::boot::{boots_directly, kernel_cmdline};

#[test]
fn a_clone_boots_the_kernel_built_with_its_very_disk() {
    assert!(boots_directly(Some("1234 1700000000\n"), "1234 1700000000", "1234 1700000000"));
}

#[test]
fn any_doubt_about_the_kernel_means_booting_through_efi() {
    // No kernel beside the image (built before kernels were kept): its own GRUB boots it.
    assert!(!boots_directly(None, "1234 1700000000", "1234 1700000000"));
    // The kernel came with another disk: its modules are not on this one.
    assert!(!boots_directly(Some("99 1600000000"), "1234 1700000000", "1234 1700000000"));
    // The image was rebuilt while it was being cloned: which one the clone holds is unknown.
    assert!(!boots_directly(Some("1234 1700000000"), "1234 1700000000", "5678 1800000000"));
    assert!(!boots_directly(Some(""), "", ""));
}

#[test]
fn the_command_line_is_one_plain_line_naming_the_root() {
    let line = "root=PARTUUID=d578426d-3a55-45ea-9d4c-1440f71724d1 ro console=hvc0 loglevel=4";
    assert_eq!(kernel_cmdline(&format!("{line}\n")).as_deref(), Some(line));
    // Without a root the kernel would panic; anything else is not what build-golden.sh wrote.
    assert_eq!(kernel_cmdline("ro console=hvc0"), None);
    assert_eq!(kernel_cmdline(""), None);
    assert_eq!(kernel_cmdline("root=/dev/vda1\nro"), None);
    assert_eq!(kernel_cmdline("root=/dev/vda1 \u{1b}[31m"), None);
    assert_eq!(kernel_cmdline(&format!("root=/dev/vda1 {}", "x".repeat(2048))), None);
}
