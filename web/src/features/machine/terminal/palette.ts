// The terminals' colors: Catppuccin, Mocha in dark mode and Latte in light mode, with the exact
// values of Ghostty's themes. Easy on the eyes for a whole day, and familiar to anyone who uses it.
import type { ITheme } from "@xterm/xterm";
import type { Theme } from "../../../lib/theme";

const MOCHA: ITheme = {
  background: "#1e1e2e",
  foreground: "#cdd6f4",
  cursor: "#f5e0dc",
  cursorAccent: "#1e1e2e",
  selectionBackground: "#585b70",
  selectionForeground: "#cdd6f4",
  black: "#45475a",
  red: "#f38ba8",
  green: "#a6e3a1",
  yellow: "#f9e2af",
  blue: "#89b4fa",
  magenta: "#f5c2e7",
  cyan: "#94e2d5",
  white: "#a6adc8",
  brightBlack: "#585b70",
  brightRed: "#f37799",
  brightGreen: "#89d88b",
  brightYellow: "#ebd391",
  brightBlue: "#74a8fc",
  brightMagenta: "#f2aede",
  brightCyan: "#6bd7ca",
  brightWhite: "#bac2de",
};

const LATTE: ITheme = {
  background: "#eff1f5",
  foreground: "#4c4f69",
  cursor: "#dc8a78",
  cursorAccent: "#eff1f5",
  selectionBackground: "#acb0be",
  selectionForeground: "#4c4f69",
  black: "#5c5f77",
  red: "#d20f39",
  green: "#40a02b",
  yellow: "#df8e1d",
  blue: "#1e66f5",
  magenta: "#ea76cb",
  cyan: "#179299",
  white: "#acb0be",
  brightBlack: "#6c6f85",
  brightRed: "#de293e",
  brightGreen: "#49af3d",
  brightYellow: "#eea02d",
  brightBlue: "#456eff",
  brightMagenta: "#fe85d8",
  brightCyan: "#2d9fa8",
  brightWhite: "#bcc0cc",
};

export const terminalTheme = (theme: Theme): ITheme => (theme === "light" ? LATTE : MOCHA);

/** Light mode: programs that draw for a dark background (Claude Code's own theme is dark, and its
 *  code is colored in RGB) would paint pale text on Latte. Every color under 4.5:1 against its
 *  cell (WCAG's bar for text) is darkened just enough to read. Off in dark mode: colors as drawn. */
export const minimumContrast = (theme: Theme): number => (theme === "light" ? 4.5 : 1);

/** The page's theme right now, as index.html and lib/theme set it. */
export const pageTheme = (): Theme => (document.documentElement.dataset.theme === "light" ? "light" : "dark");
