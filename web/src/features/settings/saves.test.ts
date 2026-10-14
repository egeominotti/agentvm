import { expect, it } from "vitest";
import { PendingSaves } from "./saves";

it("keeps newer edits pending when an earlier save completes", () => {
  const saves = new PendingSaves();
  saves.edit();
  const first = saves.start();
  expect(first).not.toBeNull();
  saves.edit();
  expect(saves.isCurrent(first!)).toBe(false);
  expect(saves.finish(first!)).toBe(false);
  const second = saves.start();
  expect(saves.isCurrent(second!)).toBe(true);
});

it("waits for the active write and coalesces edits into the next write", () => {
  const saves = new PendingSaves();
  saves.edit();
  const first = saves.start();
  saves.edit();
  expect(saves.start()).toBeNull();
  saves.edit();
  expect(saves.start()).toBeNull();
  expect(saves.finish(first!)).toBe(true);
  const latest = saves.start();
  expect(saves.isCurrent(latest!)).toBe(true);
  expect(saves.finish(latest!)).toBe(false);
});

it("does not let an old completion release a newer write", () => {
  const saves = new PendingSaves();
  saves.edit();
  const first = saves.start();
  saves.finish(first!);
  saves.edit();
  const second = saves.start();
  expect(saves.finish(first!)).toBe(false);
  expect(saves.start()).toBeNull();
  expect(saves.finish(second!)).toBe(true);
});
