/* xterm.js and its addons come from the /vendor scripts as globals; this is the one module that touches them. */

const THEME = {
  background: "#0b0c0e", foreground: "#e4e5e9", cursor: "#7480e6", cursorAccent: "#0b0c0e", selectionBackground: "#2c3160",
  black: "#151821", red: "#ff6b6b", green: "#4fd18b", yellow: "#ffb547", blue: "#7aa7ff", magenta: "#b9a8ff", cyan: "#5fd7d7", white: "#d5dae3",
  brightBlack: "#5c6577", brightRed: "#ff8a8a", brightGreen: "#74e0a5", brightYellow: "#ffc977", brightBlue: "#9cbcff", brightMagenta: "#cfc2ff", brightCyan: "#86e3e3", brightWhite: "#ffffff",
};

/** An xterm opened in `el`, with the fit addon and, when `webgl` and the GPU allow it, the WebGL renderer. */
export function openXterm(el, { fontSize, readOnly, webgl }) {
  const xterm = new window.Terminal({
    fontFamily: '"Geist Mono", "SF Mono", ui-monospace, Menlo, monospace', fontSize, lineHeight: 1.15,
    cursorBlink: !readOnly, disableStdin: readOnly, macOptionIsMeta: true, macOptionClickForcesSelection: true,
    scrollback: 5000, theme: THEME,
  });
  const fitter = new window.FitAddon.FitAddon();
  xterm.loadAddon(fitter);
  xterm.open(el);
  if (webgl) {
    try { const gl = new window.WebglAddon.WebglAddon(); gl.onContextLoss(() => gl.dispose()); xterm.loadAddon(gl); } catch {}
  }
  return { xterm, fitter };
}
