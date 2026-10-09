import { expect, it } from "vitest";
import { notLaunched } from "./prompts";

it("puts back, one per line, the tasks that were not launched", () => {
  expect(notLaunched(["a", "b", "c"], 1)).toBe("b\nc");
  expect(notLaunched(["a", "b", "c"], 3)).toBe("");
  expect(notLaunched([""], 0)).toBe("");
});
