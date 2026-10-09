import { expect, it } from "vitest";
import { task } from "../../test/task";
import { bootSteps, setupFailed } from "./boot";

it("turns the boot log into real steps with real timings", () => {
  const t = task({
    ready: false,
    status: { state: "running" },
    boot_log: ["host: disk ready in 97 ms", "[1.64s] job start", "[1.68s] repo ready on agent/x"],
  });
  const { steps, current } = bootSteps(t);
  expect(steps.map((s) => s.label)).toEqual([
    "VM slot reserved",
    "Disk cloned from the image",
    "Debian booted",
    "Repository checked out",
    "Network up",
    "Claude Code ready",
  ]);
  expect(steps[1]?.detail).toBe("97 ms");
  expect(steps[2]?.detail).toBe("1.6 s");
  expect(current).toBe(4);
});

it("a queued machine waits for a slot", () => {
  const { steps, current } = bootSteps(task({ status: { state: "queued" }, ready: false }));
  expect(current).toBe(0);
  expect(steps[0]?.detail).toBe("waiting for a free slot");
});

it("a failing setup.sh is shown as failed", () => {
  const t = task({ ready: false, boot_log: ["[2.0s] running .agentvm/setup.sh", "[3.0s] setup failed (exit 1)"] });
  const setup = bootSteps(t).steps.find((s) => s.label.includes("setup.sh"));
  expect(setup).toMatchObject({ done: true, failed: true, detail: "failed, see setup.log" });
});

it("an automatic task is ready once its network is", () => {
  const t = task({ interactive: false, ready: false, boot_log: ["[1.0s] job start", "[1.2s] network ready"] });
  expect(bootSteps(t).steps.at(-1)).toMatchObject({ label: "Agent started", done: true });
});

it("tells when the repository's setup failed, even once the machine is ready", () => {
  expect(
    setupFailed(task({ boot_log: ["[1.69s] running .agentvm/setup.sh", "[1.70s] setup failed (see setup.log)"] })),
  ).toBe(true);
  expect(setupFailed(task({ boot_log: ["[1.69s] running .agentvm/setup.sh", "[3.0s] setup done"] }))).toBe(false);
  expect(setupFailed(task())).toBe(false);
});
