// The diagnostics of a machine as a reader needs them: which log to open first, and the whole
// report as plain text for a bug report or a message.
import type { Diagnostics } from "../../../api/generated/Diagnostics";

/** The log the hint points at is the one worth opening first. */
export function openFirst(d: Diagnostics): string | undefined {
  const hint = (d.hint ?? "").toLowerCase();
  const wanted = hint.includes("setup.sh")
    ? "share/setup.log"
    : hint.includes("console")
      ? "console.log"
      : hint.includes("claude's errors")
        ? "share/claude.err"
        : "share/job.log";
  return d.logs.some((l) => l.file === wanted) ? wanted : d.logs[0]?.file;
}

export function reportText(id: string, d: Diagnostics): string {
  const lines = [`agentvm diagnostics — machine ${id}`, "", `Why: ${d.summary}`];
  if (d.hint) lines.push(`What to do: ${d.hint}`);
  lines.push("", "Timeline:", ...d.timeline.map((s) => `  ${new Date(s.at * 1000).toISOString()}  ${s.state}`));
  for (const l of d.logs) lines.push("", `--- ${l.name} (${l.file}) ---`, l.tail);
  if (d.server_log.length) lines.push("", "--- Server log ---", ...d.server_log);
  return lines.join("\n");
}
