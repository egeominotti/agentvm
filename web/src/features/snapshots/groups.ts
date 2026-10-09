// Snapshots as people look for them: by the machine they came from, a kept disk of an interrupted
// machine first, the automatic ones apart.
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";

export type SnapshotGroup = {
  task: string;
  title: string;
  repo: string;
  latest: number;
  interrupted: SnapshotMeta[];
  manual: SnapshotMeta[];
  auto: SnapshotMeta[];
};

const INTERRUPTED = /^Interrupted: /;
/** Automatic names end with why and when: "<title>, auto 10:02", "<title>, before close 10:02". */
const AUTO = /, (auto|before close) \d{1,2}:\d{2}$/;

/** The machine's title inside a snapshot's name. */
const baseTitle = (name: string) => name.replace(INTERRUPTED, "").replace(AUTO, "");

export function groupSnapshots(list: SnapshotMeta[]): SnapshotGroup[] {
  const groups = new Map<string, SnapshotGroup>();
  const newest = list.toSorted((a, b) => b.created_at - a.created_at);
  for (const s of newest) {
    let g = groups.get(s.source_task);
    if (!g) {
      g = { task: s.source_task, title: "", repo: s.repo, latest: s.created_at, interrupted: [], manual: [], auto: [] };
      groups.set(s.source_task, g);
    }
    if (INTERRUPTED.test(s.name)) g.interrupted.push(s);
    else if (s.auto) g.auto.push(s);
    else g.manual.push(s);
  }
  for (const g of groups.values()) {
    // Generated names carry the machine's title; a name typed by hand may not.
    const named = g.interrupted[0] ?? g.auto[0] ?? g.manual.at(-1);
    g.title = named ? baseTitle(named.name) : g.task;
  }
  return [...groups.values()];
}

/** What tells a snapshot apart within its machine: not the machine's title again, not the time
 *  (shown beside it). */
export function labelOf(s: SnapshotMeta, title: string): string {
  if (INTERRUPTED.test(s.name)) return "Interrupted";
  const why = s.name.match(AUTO)?.[1];
  if (why) return why === "auto" ? "Automatic" : "Before closing";
  return s.name === title ? "Snapshot" : s.name;
}
