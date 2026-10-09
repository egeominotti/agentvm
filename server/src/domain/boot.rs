//! How a VM starts: straight into the kernel kept beside the golden image, or through EFI and GRUB
//! from its own disk. Going straight in skips the firmware and GRUB, about 0.7 s of every launch,
//! but only the kernel built with that very disk can boot it: its modules live on the disk.

/// Whether a fresh clone of the golden image boots the kernel kept beside it. `stamp` names the
/// disk the kernel was built with; `before` and `after` name the image read just before and just
/// after the clone, so an image rebuilt meanwhile never pairs a kernel with the wrong disk.
pub fn boots_directly(stamp: Option<&str>, before: &str, after: &str) -> bool {
    stamp.is_some_and(|s| !before.is_empty() && s.trim() == before && before == after)
}

/// The kernel's command line kept beside the image: one plain line that names the root partition.
/// Anything else is not what the image's build wrote, and the VM boots through EFI instead.
pub fn kernel_cmdline(text: &str) -> Option<String> {
    let line = text.trim();
    let plain = line.len() <= 1024 && line.chars().all(|c| c.is_ascii_graphic() || c == ' ');
    (plain && line.split(' ').any(|w| w.starts_with("root="))).then(|| line.to_owned())
}
