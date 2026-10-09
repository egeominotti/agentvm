// The one module that opens xterm.js: theme, font, the fit addon and, when the GPU allows it,
// the WebGL renderer.
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

const THEME = {
  background: "#0b0c0e",
  foreground: "#e4e5e9",
  cursor: "#7480e6",
  cursorAccent: "#0b0c0e",
  selectionBackground: "#2c3160",
  black: "#151821",
  red: "#ff6b6b",
  green: "#4fd18b",
  yellow: "#ffb547",
  blue: "#7aa7ff",
  magenta: "#b9a8ff",
  cyan: "#5fd7d7",
  white: "#d5dae3",
  brightBlack: "#5c6577",
  brightRed: "#ff8a8a",
  brightGreen: "#74e0a5",
  brightYellow: "#ffc977",
  brightBlue: "#9cbcff",
  brightMagenta: "#cfc2ff",
  brightCyan: "#86e3e3",
  brightWhite: "#ffffff",
};

/** The terminal font, loaded before any terminal opens: xterm measures its cells once, and the
 *  GPU's glyph atlas is drawn from that measure. */
export const monoFont = (): Promise<unknown> =>
  document.fonts.check('13px "Geist Mono"') ? Promise.resolve() : document.fonts.load('13px "Geist Mono"');

/** Every terminal draws on the GPU (WebGL). When the browser takes the GPU context back (too
 *  many at once, sleep, a driver reset) the terminal falls back to the DOM renderer at once and
 *  gets the GPU again a moment later. */
export function openXterm(el: HTMLElement, { fontSize = 13, readOnly = false } = {}) {
  const xterm = new Terminal({
    fontFamily: '"Geist Mono", "SF Mono", ui-monospace, Menlo, monospace',
    fontSize,
    lineHeight: 1.15,
    cursorBlink: !readOnly,
    disableStdin: readOnly,
    macOptionIsMeta: true,
    macOptionClickForcesSelection: true,
    scrollback: 5000,
    smoothScrollDuration: 0,
    theme: THEME,
  });
  const fit = new FitAddon();
  xterm.loadAddon(fit);
  xterm.open(el);
  let losses = 0;
  let retry: ReturnType<typeof setTimeout> | undefined;
  const gpu = () => {
    try {
      const gl = new WebglAddon();
      gl.onContextLoss(() => {
        gl.dispose();
        // A context lost again and again (no GPU to give) stays on the DOM renderer.
        if (++losses <= 5) retry = setTimeout(gpu, 1000 * losses);
      });
      xterm.loadAddon(gl);
    } catch {
      // No WebGL here: the DOM renderer is slower but always there.
    }
  };
  gpu();
  /** Disposes the terminal and its pending GPU retry. */
  const close = () => {
    clearTimeout(retry);
    xterm.dispose();
  };
  return { xterm, fit, close };
}

/** Text selected in tmux arrives as OSC 52 (base64): it goes to the Mac's clipboard. */
export function copySelections(xterm: Terminal, onCopied: (ok: boolean) => void) {
  xterm.parser.registerOscHandler(52, (data) => {
    const b64 = data.slice(data.indexOf(";") + 1);
    if (!b64 || b64 === "?") return true;
    try {
      const text = new TextDecoder().decode(Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)));
      navigator.clipboard.writeText(text).then(
        () => onCopied(true),
        () => onCopied(false),
      );
    } catch {
      // Not base64: nothing to copy.
    }
    return true;
  });
}
