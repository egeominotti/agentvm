// @vitest-environment node
import { Font } from "text-shaper";
import { describe, expect, it } from "vitest";
import { DRAWN_SYMBOLS, RESTTY_FONTS } from "./restty-fonts";

// The files the dashboard serves, read with the same font reader restty uses.
const files = import.meta.glob<string>("../../../../public/fonts/*.ttf", {
  query: "?inline",
  import: "default",
  eager: true,
});
const fonts = RESTTY_FONTS.map(({ url }) => {
  const data = Object.entries(files).find(([path]) => path.endsWith(url))?.[1];
  if (!data) throw new Error(`${url} is not in public/`);
  const bytes = Uint8Array.from(atob(data.slice(data.indexOf(",") + 1)), (c) => c.charCodeAt(0));
  return Font.load(bytes.buffer);
});

describe("restty's fonts", () => {
  it("draw every symbol Claude Code and the shell print (no boxes)", () => {
    const missing = [...DRAWN_SYMBOLS].filter((c) => !fonts.some((f) => f.glyphId(c.codePointAt(0) ?? 0) !== 0));
    expect(missing.join("")).toBe("");
  });
});
