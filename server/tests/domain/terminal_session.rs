//! The terminals a VM has: Claude, the shell, and up to eight more shells opened beside it.

use agentvm::domain::terminal_session::{is_extra_shell, is_session};

#[test]
fn claude_the_shell_and_numbered_shells_are_sessions() {
    for name in ["claude", "shell", "shell-2", "shell-9"] {
        assert!(is_session(name), "{name}");
    }
    for name in ["", "bash", "shell-1", "shell-10", "shell-0", "shell-", "shell-2 ", "Shell-2", "../claude", "shell-02"]
    {
        assert!(!is_session(name), "{name:?}");
    }
}

/// Only the extra shells can be closed: Claude and the first shell are the machine's own.
#[test]
fn only_the_extra_shells_can_be_closed() {
    assert!(is_extra_shell("shell-2") && is_extra_shell("shell-9"));
    assert!(!is_extra_shell("shell") && !is_extra_shell("claude") && !is_extra_shell("shell-10"));
}
