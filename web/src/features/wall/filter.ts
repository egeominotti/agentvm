// What the Machines page lists: what each live machine is doing, how each finished one ended,
// and the ones a search and a filter keep.
import type { TaskDto } from "../../api/generated/TaskDto";
import { titleOf } from "../../lib/task";

export type LiveKind = "waiting" | "working" | "starting" | "ready";
export type EndedKind = "done" | "no_changes" | "failed" | "stopped";

export function liveKind(t: TaskDto): LiveKind {
  if (t.status.state !== "running" || (t.interactive && !t.ready)) return "starting";
  if (!t.interactive || t.activity === "working") return "working";
  return t.activity === "waiting" ? "waiting" : "ready";
}

export function endedKind(t: TaskDto): EndedKind {
  const s = t.status.state;
  return s === "done" || s === "no_changes" || s === "failed" ? s : "stopped";
}

/** Holds `query` (any case) in its title, repository, branch or id. */
const matches = (t: TaskDto, query: string) => {
  const q = query.trim().toLowerCase();
  return !q || `${titleOf(t)} ${t.repo} ${t.branch} ${t.id}`.toLowerCase().includes(q);
};

export function filterLive(list: TaskDto[], query: string, kind: LiveKind | "all"): TaskDto[] {
  return list.filter((t) => (kind === "all" || liveKind(t) === kind) && matches(t, query));
}

export function filterEnded(list: TaskDto[], query: string, kind: EndedKind | "all"): TaskDto[] {
  return list.filter((t) => (kind === "all" || endedKind(t) === kind) && matches(t, query));
}
