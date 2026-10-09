import { expect, it } from "vitest";
import { ago } from "./format";

it("says how long ago, in the largest unit that fits", () => {
  const now = 1_800_000_000;
  expect(ago(now - 20, now)).toBe("just now");
  expect(ago(now - 5 * 60, now)).toBe("5 min ago");
  expect(ago(now - 3 * 3600, now)).toBe("3 h ago");
  expect(ago(now - 2 * 86400, now)).toBe("2 days ago");
  expect(ago(now - 86400, now)).toBe("yesterday");
});
