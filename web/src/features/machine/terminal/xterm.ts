// The one module that opens xterm.js: theme, font, the fit addon and, when the GPU allows it,
// the WebGL renderer.
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { pageTheme, terminalTheme } from "./palette";

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
    theme: terminalTheme(pageTheme()),
  });
  // Light or dark follows the page, at once, also for the terminals already open.
  const recolor = new MutationObserver(() => {
    xterm.options.theme = terminalTheme(pageTheme());
  });
  recolor.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  const fit = new FitAddon();
  xterm.loadAddon(fit);
  xterm.open(el);
  let losses = 0;
  let closing = false;
  /** The canvases the WebGL renderers drew on: their contexts are given back on close. */
  const gpuCanvases = new Set<HTMLCanvasElement>();
  let retry: ReturnType<typeof setTimeout> | undefined;
  const gpu = () => {
    try {
      const gl = new WebglAddon();
      gl.onContextLoss(() => {
        gl.dispose();
        // A context lost again and again (no GPU to give) stays on the DOM renderer.
        if (!closing && ++losses <= 5) retry = setTimeout(gpu, 1000 * losses);
      });
      const before = new Set(el.querySelectorAll("canvas"));
      xterm.loadAddon(gl);
      for (const c of el.querySelectorAll("canvas")) if (!before.has(c)) gpuCanvases.add(c);
    } catch {
      // No WebGL here: the DOM renderer is slower but always there.
    }
  };
  gpu();
  /** Disposes the terminal, giving its GPU context back at once: the WebGL addon leaves it to
   *  the garbage collector, and a page with too many alive loses the oldest, which may be the
   *  terminal you are typing in. */
  const close = () => {
    closing = true;
    recolor.disconnect();
    clearTimeout(retry);
    // Only canvases that already hold a WebGL context: asking another one would create it.
    for (const canvas of gpuCanvases) canvas.getContext("webgl2")?.getExtension("WEBGL_lose_context")?.loseContext();
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
