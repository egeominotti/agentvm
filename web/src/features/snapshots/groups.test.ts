import { expect, it } from "vitest";
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";
import { groupSnapshots, labelOf } from "./groups";

const snap = (over: Partial<SnapshotMeta>): SnapshotMeta => ({
  id: "s",
  name: "Fix the cart",
  source_task: "t1",
  repo: "/Users/me/shop",
  base_sha: "abc",
  model: "sonnet",
  claude_version: null,
  created_at: 100,
  size_mb: 3000,
  cpus: 4,
  memory_mb: 4096,
  auto: false,
  compacting: false,
  ...over,
});

it("groups snapshots by machine, the machine with the newest one first", () => {
  const groups = groupSnapshots([
    snap({ id: "a", source_task: "t1", created_at: 100 }),
    snap({ id: "b", source_task: "t2", name: "Write docs", created_at: 300 }),
    snap({ id: "c", source_task: "t1", created_at: 200, auto: true, name: "Fix the cart, auto 10:02" }),
  ]);
  expect(groups.map((g) => g.task)).toEqual(["t2", "t1"]);
  const t1 = groups[1];
  expect(t1?.title).toBe("Fix the cart");
  expect(t1?.manual.map((s) => s.id)).toEqual(["a"]);
  expect(t1?.auto.map((s) => s.id)).toEqual(["c"]);
});

it("puts a disk kept from an interrupted machine first, to resume it", () => {
  const [g] = groupSnapshots([
    snap({ id: "a", created_at: 100 }),
    snap({ id: "i", created_at: 50, name: "Interrupted: Fix the cart" }),
  ]);
  expect(g?.interrupted.map((s) => s.id)).toEqual(["i"]);
  expect(g?.title).toBe("Fix the cart");
});

it("names a snapshot without repeating its machine or the time shown beside it", () => {
  expect(labelOf(snap({ name: "Fix the cart" }), "Fix the cart")).toBe("Snapshot");
  expect(labelOf(snap({ name: "Fix the cart, auto 10:02", auto: true }), "Fix the cart")).toBe("Automatic");
  expect(labelOf(snap({ name: "Fix the cart, before close 10:02", auto: true }), "Fix the cart")).toBe(
    "Before closing",
  );
  expect(labelOf(snap({ name: "with snap.txt" }), "Fix the cart")).toBe("with snap.txt");
  expect(labelOf(snap({ name: "Interrupted: Fix the cart" }), "Fix the cart")).toBe("Interrupted");
});
