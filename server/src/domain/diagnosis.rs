//! From a failure to what to do about it: the known causes, each with its remedy.

/// A sentence telling the user what went wrong and what to do, for the failure `reason` (with
/// the guest's job log for causes it records there). `None` for a cause it does not know.
pub fn hint(reason: &str, job_log: &str) -> Option<String> {
    let cause = cause(reason, job_log)?;
    Some(if reason.contains("Its disk is kept in Snapshots") {
        format!("{cause} Its disk was kept: restore it from Snapshots to continue where it stopped.")
    } else {
        cause.to_owned()
    })
}

fn cause(reason: &str, job_log: &str) -> Option<&'static str> {
    let r = reason.to_lowercase();
    Some(if r.contains("no space left") || r.contains("disk is full") || r.contains("gb free") {
        "The Mac's disk is full: free some space, then launch it again."
    } else if job_log.contains("setup failed") {
        "The repository's .agentvm/setup.sh failed: its output is in setup.log below."
    } else if r.starts_with("claude_install_failed") {
        "The Claude Code version asked for could not be installed: check the version, or the VM's network."
    } else if r.starts_with("timeout") {
        "The task ran out of time (Settings → time limit). What it committed is on its branch."
    } else if r.starts_with("vm_error") {
        "The VM itself failed (at boot, or it crashed): the console below shows how far it got."
    } else if r.starts_with("guest_error") || r.starts_with("guest_no_result") {
        "The job runner inside the VM stopped with an error: the job log below shows where."
    } else if r.starts_with("fetch_failed") {
        "The work could not be imported into the repository: the server log below says why."
    } else if r.contains("token") {
        "No Claude token: set one in Settings (claude setup-token)."
    } else if r.starts_with("interrupted while preparing") {
        "The server stopped while preparing this VM: launch it again."
    } else if r.starts_with("claude_exit") {
        "Claude stopped with an error: Claude's errors below say why."
    } else {
        return None;
    })
}
