import { expect, it } from "vitest";
import { parsePref, resolveTheme } from "./theme";

it("follows the system unless light or dark was chosen", () => {
  expect(resolveTheme("system", true)).toBe("dark");
  expect(resolveTheme("system", false)).toBe("light");
  expect(resolveTheme("light", true)).toBe("light");
  expect(resolveTheme("dark", false)).toBe("dark");
});

it("reads only a known choice back; anything else is the system's", () => {
  expect(parsePref("light")).toBe("light");
  expect(parsePref("dark")).toBe("dark");
  expect(parsePref("system")).toBe("system");
  expect(parsePref(null)).toBe("system");
  expect(parsePref("purple")).toBe("system");
});
