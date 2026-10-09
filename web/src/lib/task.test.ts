import { describe, expect, it } from "vitest";
import { task } from "../test/task";
import { isEnded, shortId, statusOf, titleOf } from "./task";

describe("statusOf", () => {
  it("names each moment of a machine's life the same way everywhere", () => {
    expect(statusOf(task({ status: { state: "queued" } }))).toEqual({ tone: "boot", label: "Queued" });
    expect(statusOf(task({ status: { state: "booting" } }))).toEqual({ tone: "boot", label: "Starting" });
    expect(statusOf(task({ ready: false }))).toEqual({ tone: "boot", label: "Starting" });
    expect(statusOf(task({ activity: "working" }))).toEqual({ tone: "work", label: "Working" });
    expect(statusOf(task({ activity: "waiting" }))).toEqual({ tone: "wait", label: "Waiting for you" });
    expect(statusOf(task())).toEqual({ tone: "idle", label: "Ready" });
    expect(statusOf(task({ status: { state: "collecting" } }))).toEqual({ tone: "boot", label: "Saving" });
    expect(statusOf(task({ status: { state: "failed", reason: "boom" } }))).toEqual({ tone: "fail", label: "Failed" });
    expect(statusOf(task({ status: { state: "stopped" } }))).toEqual({ tone: "stop", label: "Stopped" });
  });

  it("counts the commits a finished machine brought back", () => {
    expect(statusOf(task({ status: { state: "done", branch: "agent/x", commits: 1 } })).label).toBe("Done, 1 commit");
    expect(statusOf(task({ status: { state: "done", branch: "agent/x", commits: 3 } })).label).toBe("Done, 3 commits");
  });

  it("shows an automatic machine as working while it runs, without a terminal to be ready", () => {
    expect(statusOf(task({ interactive: false, ready: false }))).toEqual({ tone: "work", label: "Working" });
  });
});

describe("a machine's name", () => {
  it("is the first line of its task, else its label, else its repository", () => {
    expect(titleOf(task({ prompt: "Fix the cart\nand the tests" }))).toBe("Fix the cart");
    expect(titleOf(task({ label: "Spike" }))).toBe("Spike");
    expect(titleOf(task())).toBe("shop");
  });

  it("is told apart from a twin by the end of its id", () => {
    expect(shortId(task())).toBe("794b");
  });
});

it("a machine has ended once done, without changes, failed or stopped", () => {
  expect(isEnded(task())).toBe(false);
  expect(isEnded(task({ status: { state: "no_changes" } }))).toBe(true);
});
