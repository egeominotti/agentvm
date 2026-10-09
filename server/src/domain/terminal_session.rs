//! The terminals of a VM, each a tmux session inside it: `claude`, `shell`, and the extra shells
//! opened beside them, `shell-2` to `shell-9`.

/// Extra shells a VM can have beside the first one.
pub const EXTRA_SHELLS: u8 = 8;

pub fn is_session(name: &str) -> bool {
    name == "claude" || name == "shell" || is_extra_shell(name)
}

/// `shell-2` … `shell-9`: the shells that can be opened and closed. Claude and the first shell
/// are the machine's own.
pub fn is_extra_shell(name: &str) -> bool {
    name.strip_prefix("shell-")
        .and_then(|n| (n.len() == 1).then(|| n.parse::<u8>().ok()).flatten())
        .is_some_and(|n| (2..2 + EXTRA_SHELLS).contains(&n))
}
