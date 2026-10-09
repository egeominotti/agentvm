// What the Snapshots table shows: the kind of each snapshot, and the ones a search and a kind keep.
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";

export type Kind = "manual" | "auto" | "interrupted";
export type KindFilter = Kind | "all";

/** Interrupted: the disk of a machine that ended without its work saved, kept to resume it. */
export function kindOf(s: SnapshotMeta): Kind {
  if (s.name.startsWith("Interrupted: ")) return "interrupted";
  return s.auto ? "auto" : "manual";
}

/** Those whose name or repository holds `query` (any case), of `kind`. */
export function filterSnapshots(list: SnapshotMeta[], query: string, kind: KindFilter): SnapshotMeta[] {
  const q = query.trim().toLowerCase();
  return list.filter(
    (s) => (kind === "all" || kindOf(s) === kind) && (!q || `${s.name} ${s.repo}`.toLowerCase().includes(q)),
  );
}
