import { createHeadlessTerminal } from "restty/headless";
import { describe, expect, test } from "vitest";
import { terminalTheme } from "../machine/terminal/palette";
import { extent, lineHtml, themeColors } from "./screen";

const LATTE = themeColors(terminalTheme("light"));

/** A real terminal core (Ghostty's, as the previews use) after `text`, and its cells. */
async function screen(text: string, cols = 30, rows = 4) {
  const term = await createHeadlessTerminal({ cols, rows, replay: false, maxScrollbackBytes: 0 });
  term.wasm.setDefaultColors(term.handle, LATTE.fg, LATTE.bg, LATTE.cursor);
  term.wasm.setPalette(term.handle, LATTE.palette, 16);
  term.write(text);
  term.renderUpdate();
  const cells = term.snapshot();
  term.dispose();
  if (!cells) throw new Error("no cells");
  return cells;
}

describe("a preview's lines", () => {
  test("plain text has no markup and no trailing blanks", async () => {
    const s = await screen("hello world");
    expect(lineHtml(s, 0, LATTE)).toBe("hello world");
    expect(lineHtml(s, 1, LATTE)).toBe("");
  });

  test("a colored, bold run is one span in the theme's color", async () => {
    const s = await screen("ok \x1b[1;31mfailed\x1b[0m done");
    expect(lineHtml(s, 0, LATTE)).toBe('ok <span style="color:#d20f39;font-weight:700">failed</span> done');
  });

  test("text is escaped, never markup", async () => {
    const s = await screen("<img src=x onerror=alert(1)> & co", 40);
    expect(lineHtml(s, 0, LATTE)).toBe("&lt;img src=x onerror=alert(1)&gt; &amp; co");
  });

  test("a wide character takes its two cells once", async () => {
    const s = await screen("a世界b");
    expect(lineHtml(s, 0, LATTE)).toBe("a世界b");
  });

  test("inverse swaps the colors, a background is kept", async () => {
    const s = await screen("\x1b[7m> \x1b[0m \x1b[44m add \x1b[0m");
    expect(lineHtml(s, 0, LATTE)).toBe(
      '<span style="color:#eff1f5;background:#4c4f69">&gt; </span> <span style="background:#1e66f5"> add </span>',
    );
  });

  test("the drawn area ends at the last character or colored cell", async () => {
    const s = await screen("top\r\n\r\n  \x1b[42m    \x1b[0m", 40, 10);
    expect(extent(s, LATTE)).toEqual({ cols: 6, rows: 3 });
    expect(extent(await screen("", 40, 10), LATTE)).toEqual({ cols: 0, rows: 0 });
  });
});

test("the theme's colors as the terminal core takes them", () => {
  expect(LATTE.fg).toBe(0x4c4f69);
  expect(LATTE.bg).toBe(0xeff1f5);
  expect(Array.from(LATTE.palette.subarray(3, 6))).toEqual([0xd2, 0x0f, 0x39]);
  expect(LATTE.palette.length).toBe(48);
});
