import { expect, it } from "vitest";
import { explain, readableReason } from "./explain";

it("tells a VM that never started from one that died while working", () => {
  expect(explain("vm_error: Virtualization.framework: invalid configuration")[0]).toBe("The VM did not start");
  expect(explain("vm_error: agentvm-vm exited without a final event\n--- console ---\nlogin:")[0]).toBe(
    "The VM stopped unexpectedly",
  );
  expect(explain("timeout")[0]).toBe("Time limit reached");
  expect(explain("something new")[0]).toBe("Failed");
});

it("shows a reason without the console's control codes, and without the kept-disk note", () => {
  const raw =
    'vm_error: agentvm-vm exited without a final event\n--- console ---\n\u001b[!p\u001b]104\u0007\u001b[?7h\u001b[6n\u001b[32766;32766H\r\nDebian GNU/Linux 13 agentvm hvc0\n\nagentvm login: . Its disk is kept in Snapshots as "Interrupted: setup-fail, 03:21"';
  const text = readableReason(raw);
  for (const control of ["\u001b", "\u0007", "\r"]) expect(text.includes(control)).toBe(false);
  expect(text).toContain("Debian GNU/Linux 13 agentvm hvc0");
  expect(text).not.toContain("Its disk is kept");
});
