import { expect, it } from "vitest";
import { task } from "../../test/task";
import { endedKind, filterEnded, filterLive, liveKind } from "./filter";

it("tells what a live machine is doing", () => {
  expect(liveKind(task({ status: { state: "running" }, interactive: true, ready: true, activity: "waiting" }))).toBe(
    "waiting",
  );
  expect(liveKind(task({ status: { state: "running" }, interactive: true, ready: true, activity: "working" }))).toBe(
    "working",
  );
  expect(liveKind(task({ status: { state: "running" }, interactive: true, ready: false }))).toBe("starting");
  expect(liveKind(task({ status: { state: "queued" } }))).toBe("starting");
  expect(liveKind(task({ status: { state: "running" }, interactive: true, ready: true, activity: null }))).toBe(
    "ready",
  );
  expect(liveKind(task({ status: { state: "running" }, interactive: false, ready: true }))).toBe("working");
});

it("finds live machines by title, repository, branch or id, in one state or all", () => {
  const a = task({
    id: "01a1-aaaa",
    prompt: "Fix the cart",
    repo: "/r/shop",
    status: { state: "running" },
    ready: true,
    activity: "waiting",
  });
  const b = task({
    id: "01a1-bbbb",
    prompt: "Write docs",
    repo: "/r/docs-site",
    status: { state: "running" },
    ready: true,
    activity: "working",
  });
  const ids = (l: { id: string }[]) => l.map((t) => t.id);
  expect(ids(filterLive([a, b], "", "all"))).toEqual(["01a1-aaaa", "01a1-bbbb"]);
  expect(ids(filterLive([a, b], "docs", "all"))).toEqual(["01a1-bbbb"]);
  expect(ids(filterLive([a, b], "bbbb", "all"))).toEqual(["01a1-bbbb"]);
  expect(ids(filterLive([a, b], "", "waiting"))).toEqual(["01a1-aaaa"]);
  expect(ids(filterLive([a, b], "cart", "working"))).toEqual([]);
});

it("sorts out how finished machines ended", () => {
  expect(endedKind(task({ status: { state: "done", branch: "agent/x", commits: 2 } }))).toBe("done");
  expect(endedKind(task({ status: { state: "no_changes" } }))).toBe("no_changes");
  expect(endedKind(task({ status: { state: "failed", reason: "x" } }))).toBe("failed");
  expect(endedKind(task({ status: { state: "stopped" } }))).toBe("stopped");
  const done = task({ id: "d", status: { state: "done", branch: "agent/x", commits: 1 } });
  const failed = task({ id: "f", status: { state: "failed", reason: "x" } });
  expect(filterEnded([done, failed], "", "failed").map((t) => t.id)).toEqual(["f"]);
});
