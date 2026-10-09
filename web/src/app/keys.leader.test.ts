import { expect, it } from "vitest";
import { goTarget } from "./keys";

it("goes to a screen only when the letter follows g at once", () => {
  expect(goTarget(["g", "s"])).toBe("#/snapshots");
  expect(goTarget(["g", "m"])).toBe("#/wall");
  expect(goTarget(["g", ","])).toBe("#/settings");
  // "git s": other keys in between cancel the g.
  expect(goTarget(["g", "i", "t", " ", "s"])).toBeNull();
  expect(goTarget(["s"])).toBeNull();
});
