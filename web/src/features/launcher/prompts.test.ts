import { expect, it } from "vitest";
import { mergeRecent, splitPrompts } from "./prompts";

it("launches one VM with the whole text, or one per line", () => {
  expect(splitPrompts("Fix the cart\nand its tests", false)).toEqual(["Fix the cart\nand its tests"]);
  expect(splitPrompts("- Fix the cart\n2. Write the docs\n\n• Bump deps", true)).toEqual([
    "Fix the cart",
    "Write the docs",
    "Bump deps",
  ]);
});

it("an empty task still launches one VM, to work in its terminal", () => {
  expect(splitPrompts("   ", false)).toEqual([""]);
  expect(splitPrompts("\n\n", true)).toEqual([""]);
});

it("keeps recent repositories newest first, without duplicates, at most 8", () => {
  const before = ["~/a", "~/b", "~/c", "~/d", "~/e", "~/f", "~/g", "~/h"];
  expect(mergeRecent("~/c", before)).toEqual(["~/c", "~/a", "~/b", "~/d", "~/e", "~/f", "~/g", "~/h"]);
  expect(mergeRecent("~/new", before)).toHaveLength(8);
  expect(mergeRecent("~/new", before)[0]).toBe("~/new");
});
