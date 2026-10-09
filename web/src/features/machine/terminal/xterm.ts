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
    theme: THEME,
  });
  const fit = new FitAddon();
  xterm.loadAddon(fit);
  xterm.open(el);
  try {
    const gl = new WebglAddon();
    gl.onContextLoss(() => gl.dispose());
    xterm.loadAddon(gl);
  } catch {
    // The DOM renderer is slower but always there.
  }
  return { xterm, fit };
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
