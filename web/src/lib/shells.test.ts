import { expect, it } from "vitest";
import { nextShell, parseShells, sessionLabel } from "./shells";

it("names each terminal as the tabs show it", () => {
  expect(sessionLabel("claude")).toBe("Claude");
  expect(sessionLabel("shell")).toBe("Shell");
  expect(sessionLabel("shell-2")).toBe("Shell 2");
  expect(sessionLabel("shell-9")).toBe("Shell 9");
});

it("opens the lowest free extra shell, up to Shell 9", () => {
  expect(nextShell([])).toBe("shell-2");
  expect(nextShell(["shell-2", "shell-4"])).toBe("shell-3");
  expect(nextShell(["shell-2", "shell-3", "shell-4", "shell-5", "shell-6", "shell-7", "shell-8"])).toBe("shell-9");
  expect(
    nextShell(["shell-2", "shell-3", "shell-4", "shell-5", "shell-6", "shell-7", "shell-8", "shell-9"]),
  ).toBeNull();
});

it("reads back only valid extra shells, in order and once each", () => {
  expect(parseShells('["shell-4","shell-2","shell-4"]')).toEqual(["shell-2", "shell-4"]);
  expect(parseShells('["shell","claude","shell-10","x",3]')).toEqual([]);
  expect(parseShells("not json")).toEqual([]);
  expect(parseShells(null)).toEqual([]);
});
