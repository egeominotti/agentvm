// What a wall preview shows: the cells of a headless terminal (Ghostty's core, the one every
// terminal here runs) turned into lines of HTML. No GPU, no glyph atlas, no font parsed per
// preview: the browser draws the text, so a wall of many machines opens at once and scrolls freely.
import type { ITheme } from "@xterm/xterm";
import type { ResttyHeadlessSnapshot } from "restty/headless";

/** The colors the terminal core paints with, as it takes them: packed 0xRRGGBB, RGB triples. */
export type Colors = { fg: number; bg: number; cursor: number; palette: Uint8Array };

const ANSI = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
] as const;

const packed = (hex = "#000000") => Number.parseInt(hex.slice(1, 7), 16);

/** The same Catppuccin as the full terminals, for the core's default colors and 16-color palette. */
export function themeColors(t: ITheme): Colors {
  const palette = new Uint8Array(ANSI.length * 3);
  ANSI.forEach((name, i) => {
    const c = packed(t[name]);
    palette.set([c >> 16, (c >> 8) & 255, c & 255], i * 3);
  });
  return { fg: packed(t.foreground), bg: packed(t.background), cursor: packed(t.cursor), palette };
}

// The style bits Ghostty's core sets on a cell.
const BOLD = 1;
const ITALIC = 2;
const FAINT = 4;
const INVERSE = 16;
const INVISIBLE = 32;
const STRIKE = 64;
const UNDERLINE = 1792;
const WIDE_TAIL = 2;

const rgb = (bytes: Uint8Array | null, i: number, fallback: number) =>
  bytes ? (bytes[i * 4]! << 16) | (bytes[i * 4 + 1]! << 8) | bytes[i * 4 + 2]! : fallback;
export const hex = (c: number): string => `#${c.toString(16).padStart(6, "0")}`;
const ESCAPED: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;" };

/** A cell as drawn: its text, colors after inverse, and style bits. */
type Cell = { text: string; fg: number; bg: number; flags: number };

function cellAt(s: ResttyHeadlessSnapshot, i: number, c: Colors): Cell {
  const cp = s.codepoints?.[i] ?? 0;
  let text = cp > 32 ? String.fromCodePoint(cp) : " ";
  const extra = s.graphemeLen?.[i] ?? 0;
  if (extra && s.graphemeBuffer && s.graphemeOffset) {
    const from = s.graphemeOffset[i]!;
    text += String.fromCodePoint(...s.graphemeBuffer.subarray(from, from + extra));
  }
  const flags = s.styleFlags?.[i] ?? 0;
  const fg = rgb(s.fgBytes, i, c.fg);
  const bg = rgb(s.bgBytes, i, c.bg);
  return flags & INVERSE ? { text, fg: bg, bg: fg, flags } : { text, fg, bg, flags };
}

const plain = (x: Cell, c: Colors) => x.text === " " && x.bg === c.bg && !(x.flags & (INVERSE | UNDERLINE | STRIKE));
const sameLook = (a: Cell, b: Cell) => a.fg === b.fg && a.bg === b.bg && a.flags === b.flags;

/** The inline style of a run: empty when it is plain text in the default colors. */
function style(x: Cell, c: Colors): string {
  const css: string[] = [];
  if (x.flags & INVISIBLE) css.push("color:transparent");
  else if (x.fg !== c.fg) css.push(`color:${hex(x.fg)}`);
  if (x.bg !== c.bg) css.push(`background:${hex(x.bg)}`);
  if (x.flags & BOLD) css.push("font-weight:700");
  if (x.flags & ITALIC) css.push("font-style:italic");
  if (x.flags & FAINT) css.push("opacity:.6");
  const lines = [x.flags & UNDERLINE ? "underline" : "", x.flags & STRIKE ? "line-through" : ""].filter(Boolean);
  if (lines.length) css.push(`text-decoration:${lines.join(" ")}`);
  return css.join(";");
}

/** Row `row` as HTML: runs of one look in a span each, the blank end of the line left out. */
export function lineHtml(s: ResttyHeadlessSnapshot, row: number, c: Colors): string {
  const cells: Cell[] = [];
  for (let col = 0; col < s.cols; col += 1) {
    const i = row * s.cols + col;
    if (s.wide?.[i] !== WIDE_TAIL) cells.push(cellAt(s, i, c));
  }
  while (cells.length && plain(cells.at(-1)!, c)) cells.pop();
  let out = "";
  for (let from = 0; from < cells.length;) {
    let to = from + 1;
    while (to < cells.length && sameLook(cells[to]!, cells[from]!)) to += 1;
    const text = cells
      .slice(from, to)
      .map((x) => x.text)
      .join("")
      .replace(/[&<>]/g, (ch) => ESCAPED[ch]!);
    const css = style(cells[from]!, c);
    out += css ? `<span style="${css}">${text}</span>` : text;
    from = to;
  }
  return out;
}

/** How much of the screen is drawn on: up to the last character or colored cell. */
export function extent(s: ResttyHeadlessSnapshot, c: Colors): { cols: number; rows: number } {
  let cols = 0;
  let rows = 0;
  for (let row = 0; row < s.rows; row += 1) {
    for (let col = s.cols - 1; col >= 0; col -= 1) {
      if (!plain(cellAt(s, row * s.cols + col, c), c)) {
        cols = Math.max(cols, col + 1);
        rows = row + 1;
        break;
      }
    }
  }
  return { cols, rows };
}
