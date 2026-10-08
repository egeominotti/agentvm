/* What a task (a machine) is doing, as the dashboard shows it. */
import { duration, plural, repoName } from "./format.js";

const TERMINAL = new Set(["done", "no_changes", "failed", "stopped"]);

/** The machine is gone: done, closed without changes, failed or stopped. */
export const isEnded = t => TERMINAL.has(t.status.state);
/** Claude finished a turn and waits for the user. */
export const isWaiting = t => t.status.state === "running" && t.activity === "waiting";

export const age = t => duration((t.finished_at ?? Date.now() / 1000) - t.created_at);
export const titleOf = t => (t.prompt ? t.prompt.split("\n")[0] : t.label || repoName(t.repo));

/** Visual state of a machine: [css class, label]. */
export function kind(t) {
  const s = t.status.state;
  if (s === "queued") return ["k-boot", "Queued"];
  if (s === "preparing" || s === "booting") return ["k-boot", "Booting"];
  if (s === "collecting") return ["k-boot", "Saving"];
  if (s === "running") {
    if (t.interactive && !t.ready) return ["k-boot", "Booting"];
    if (!t.interactive || t.activity === "working") return ["k-work", "Working"];
    if (t.activity === "waiting") return ["k-wait", "Waiting for you"];
    return ["k-idle", "Ready"];
  }
  if (s === "done") return ["k-done", `Done, ${plural(t.status.commits, "commit")}`];
  if (s === "no_changes") return ["k-done", "No changes"];
  if (s === "stopped") return ["k-stop", "Stopped"];
  return ["k-fail", "Failed"];
}
