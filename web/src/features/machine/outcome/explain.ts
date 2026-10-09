// A failure in words: a title and what it means, and the server's reason made readable.

/** [title, explanation] for a failure reason reported by the server. */
export function explain(reason: string): [string, string] {
  if (reason.startsWith("timeout"))
    return ["Time limit reached", "The agent was stopped after the time limit in Settings."];
  if (reason.startsWith("guest_no_result"))
    return ["The VM shut down without a result", "Diagnostics show its last logs."];
  if (reason.startsWith("claude_exit")) return ["Claude stopped with an error", "Diagnostics show what it printed."];
  if (reason.startsWith("vm_error")) {
    // It ran, then its process ended: a crash, a kill, or the Mac out of memory.
    return /exited without a final event|stopped while/.test(reason)
      ? ["The VM stopped unexpectedly", "Its process ended while it ran: a crash, or the Mac ran out of memory."]
      : ["The VM did not start", "Virtualization.framework reported an error."];
  }
  if (reason.startsWith("fetch_failed"))
    return ["Could not import the branch", "The work finished but git fetch failed."];
  return ["Failed", "Diagnostics show what happened."];
}

/** The reason without terminal control codes (the VM console's) and without the note about a
 *  kept disk, which the outcome offers as an action instead. */
export function readableReason(reason: string): string {
  return (
    reason
      .replace(/\s*\.?\s*Its disk is kept in Snapshots as ".*"\s*$/s, "")
      // biome-ignore lint/suspicious/noControlCharactersInRegex: control codes are what is removed
      .replace(/\u001b\[[0-9;?!]*[A-Za-z]|\u001b\][^\u0007]*\u0007|[\u0000-\u0008\u000b-\u001f]/g, "")
      .replace(/\n{3,}/g, "\n\n")
      .trim()
  );
}
