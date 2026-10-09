import { expect, it } from "vitest";
import type { Diagnostics } from "../../../api/generated/Diagnostics";
import { openFirst, reportText } from "./report";

const d = (hint: string | null): Diagnostics => ({
  summary: "setup_failed",
  hint,
  timeline: [{ state: "queued", at: 0 }],
  logs: [
    { name: "Job", file: "share/job.log", tail: "job" },
    { name: "Setup", file: "share/setup.log", tail: "boom" },
  ],
  server_log: ["line"],
});

it("opens first the log the hint points at", () => {
  expect(openFirst(d("Your .agentvm/setup.sh failed: see setup.log"))).toBe("share/setup.log");
  expect(openFirst(d(null))).toBe("share/job.log");
});

it("writes the whole report as plain text", () => {
  const text = reportText("abc", d("Fix setup.sh"));
  expect(text).toContain("Why: setup_failed");
  expect(text).toContain("What to do: Fix setup.sh");
  expect(text).toContain("--- Setup (share/setup.log) ---\nboom");
  expect(text).toContain("--- Server log ---\nline");
});
