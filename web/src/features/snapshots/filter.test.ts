import { expect, it } from "vitest";
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";
import { filterSnapshots, kindOf } from "./filter";

const snap = (over: Partial<SnapshotMeta>): SnapshotMeta => ({
  id: "s",
  name: "Fix the cart",
  source_task: "t1",
  repo: "/Users/me/shop",
  base_sha: "abc",
  model: "sonnet",
  claude_version: null,
  created_at: 100,
  size_mb: 30,
  cpus: 4,
  memory_mb: 4096,
  auto: false,
  compacting: false,
  ...over,
});

it("tells the kinds of snapshot apart", () => {
  expect(kindOf(snap({}))).toBe("manual");
  expect(kindOf(snap({ auto: true, name: "Fix the cart, auto 10:02" }))).toBe("auto");
  expect(kindOf(snap({ auto: true, name: "Fix the cart, before close 10:02" }))).toBe("auto");
  expect(kindOf(snap({ name: "Interrupted: demo-web, 15:31" }))).toBe("interrupted");
});

it("finds snapshots by name or repository, of one kind or all", () => {
  const all = [
    snap({ id: "a", name: "Fix the cart" }),
    snap({ id: "b", name: "Write docs", repo: "/Users/me/docs-site", auto: true }),
    snap({ id: "c", name: "Interrupted: shop, 15:31" }),
  ];
  const ids = (l: SnapshotMeta[]) => l.map((s) => s.id);
  expect(ids(filterSnapshots(all, "", "all"))).toEqual(["a", "b", "c"]);
  expect(ids(filterSnapshots(all, "docs", "all"))).toEqual(["b"]);
  expect(ids(filterSnapshots(all, "SHOP", "all"))).toEqual(["a", "c"]);
  expect(ids(filterSnapshots(all, "", "auto"))).toEqual(["b"]);
  expect(ids(filterSnapshots(all, "", "interrupted"))).toEqual(["c"]);
  expect(ids(filterSnapshots(all, "cart", "auto"))).toEqual([]);
});
