// What a machine (a task) is doing, as the dashboard shows it everywhere: one tone, one label.
import type { TaskDto } from "../api/generated/TaskDto";
import { duration, plural, repoName } from "./format";

export type Tone = "boot" | "work" | "wait" | "idle" | "done" | "fail" | "stop";

const ENDED = new Set(["done", "no_changes", "failed", "stopped"]);

/** Done, closed without changes, failed or stopped: the VM is gone. */
export const isEnded = (t: TaskDto) => ENDED.has(t.status.state);
/** Claude finished a turn and waits for the user. */
export const isWaiting = (t: TaskDto) => t.status.state === "running" && t.activity === "waiting";
export const isRunning = (t: TaskDto) => t.status.state === "running";

export const age = (t: TaskDto) => duration((t.finished_at ?? Date.now() / 1000) - t.created_at);
export const titleOf = (t: TaskDto) => (t.prompt ? (t.prompt.split("\n")[0] ?? "") : (t.label ?? repoName(t.repo)));
/** The end of the id: tells apart two machines on the same repository. */
export const shortId = (t: TaskDto) => t.id.slice(-4);

/** The machine's state: a tone for its mark, and the words for it. */
export function statusOf(t: TaskDto): { tone: Tone; label: string } {
  const s = t.status;
  switch (s.state) {
    case "queued":
      return { tone: "boot", label: "Queued" };
    case "preparing":
    case "booting":
      return { tone: "boot", label: "Starting" };
    case "collecting":
      return { tone: "boot", label: "Saving" };
    case "running":
      if (t.interactive && !t.ready) return { tone: "boot", label: "Starting" };
      if (!t.interactive || t.activity === "working") return { tone: "work", label: "Working" };
      if (t.activity === "waiting") return { tone: "wait", label: "Waiting for you" };
      return { tone: "idle", label: "Ready" };
    case "done":
      return { tone: "done", label: `Done, ${plural(s.commits, "commit")}` };
    case "no_changes":
      return { tone: "done", label: "No changes" };
    case "stopped":
      return { tone: "stop", label: "Stopped" };
    case "failed":
      return { tone: "fail", label: "Failed" };
  }
}
