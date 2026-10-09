import { expect, it } from "vitest";
import { mergeRecent } from "./recent";

it("keeps recent repositories newest first, without duplicates, at most 8", () => {
  const before = ["~/a", "~/b", "~/c", "~/d", "~/e", "~/f", "~/g", "~/h"];
  expect(mergeRecent("~/c", before)).toEqual(["~/c", "~/a", "~/b", "~/d", "~/e", "~/f", "~/g", "~/h"]);
  expect(mergeRecent("~/new", before)).toHaveLength(8);
  expect(mergeRecent("~/new", before)[0]).toBe("~/new");
});
