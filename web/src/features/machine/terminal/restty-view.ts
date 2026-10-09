// A terminal drawn by restty: Ghostty's own terminal core (libghostty-vt, WebAssembly) on WebGPU,
// or WebGL2. Loaded only when a terminal opens: the rest of the dashboard does not wait for it.
// It talks to the VM through our PtySocket (plugged in as restty's PTY transport), so it also
// answers the programs' queries (colors, size) as Ghostty does.
import type { PtyCallbacks, PtyTransport } from "restty";
import { DeviceAttributes } from "../../../lib/da-replies";
import { Osc52Scanner } from "../../../lib/osc52";
import type { TerminalView, ViewOptions } from "./engine";
import { pageTheme } from "./palette";
import { RESTTY_FONTS } from "./restty-fonts";

/** Ghostty's own Catppuccin themes, as in the user's Ghostty: Latte light, Mocha dark. */
const THEME = { light: "Catppuccin Latte", dark: "Catppuccin Mocha" } as const;

export async function openRestty(el: HTMLElement, opts: ViewOptions): Promise<TerminalView> {
  const { Restty, getBuiltinTheme } = await import("restty");
  const { socket, readOnly = false, fontSize = 13, fixed, onCopied } = opts;
  const decoder = new TextDecoder();
  const clipboard = new Osc52Scanner();
  const attributes = new DeviceAttributes();
  let callbacks: PtyCallbacks | null = null;

  // Output from the VM, decoded across chunk boundaries (a character may be split in two).
  socket.onOutput((bytes) => {
    const text = decoder.decode(bytes, { stream: true });
    attributes.output(text);
    callbacks?.onData?.(text);
  });
  socket.onOpen(() => callbacks?.onConnect?.());
  const transport: PtyTransport = {
    connect: ({ cols, rows, callbacks: cb }) => {
      callbacks = cb;
      socket.connect(fixed?.cols ?? cols ?? 0, fixed?.rows ?? rows ?? 0);
    },
    // The socket outlives restty's idea of a connection: it is closed with the view.
    disconnect: () => {},
    sendInput: (data) => {
      if (readOnly) return false;
      socket.send(attributes.reply(data));
      return true;
    },
    resize: (cols, rows) => {
      if (!fixed) socket.resize(cols, rows);
      return true;
    },
    isConnected: () => socket.isOpen(),
  };

  const create = (renderer: "auto" | "webgl2") =>
    new Restty({
      root: el,
      terminal: {
        renderer,
        fonts: RESTTY_FONTS,
        // The size of the letters, as everywhere else on the page. restty's default ("height")
        // fits the whole line height in it: letters a quarter smaller, thin and hard to read.
        fontSize,
        fontSizeMode: "em",
        // As Ghostty on macOS: blended as the system blends text, full strokes rather than the
        // thinner linear-space look.
        alphaBlending: "native",
        theme: getBuiltinTheme(THEME[pageTheme()]) ?? undefined,
        autoResize: !fixed,
        showResizeOverlay: false,
        attachCanvasEvents: !readOnly,
      },
      services: {
        ptyTransport: transport,
        // tmux puts a mouse selection on the clipboard with OSC 52: it goes to the Mac's.
        beforeRenderOutput: ({ text }) => {
          for (const copied of clipboard.push(text)) {
            navigator.clipboard.writeText(copied).then(
              () => onCopied?.(true),
              () => onCopied?.(false),
            );
          }
        },
      },
      surface: { shortcuts: false, defaultContextMenu: false, searchUi: false },
    });
  // WebGPU first. A browser may offer WebGPU and fail to draw with it (some drivers, headless):
  // restty then has no renderer at all, so WebGL2 is tried, and failing that the caller falls
  // back to xterm.js. A terminal is never left blank.
  let restty = create("auto");
  if (!(await drawing(restty))) {
    restty.destroy();
    el.replaceChildren();
    restty = create("webgl2");
    if (!(await drawing(restty))) {
      restty.destroy();
      throw new Error("restty found no GPU renderer (WebGPU, WebGL2)");
    }
  }
  restty.connectPty("agentvm");
  // Which GPU path it took, readable from the page (diagnostics, tests).
  el.dataset.engine = `restty:${restty.getBackend()}`;
  if (fixed) restty.resize(fixed.cols, fixed.rows);

  // Light or dark follows the page, at once.
  const recolor = new MutationObserver(() => {
    const theme = getBuiltinTheme(THEME[pageTheme()]);
    if (theme) restty.applyTheme(theme);
  });
  recolor.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

  return {
    engine: "restty",
    focus: () => restty.focus(),
    // Typed into the session, as if the user typed it (dropped files' paths).
    paste: (text) => {
      if (!readOnly) socket.send(text);
      restty.focus();
    },
    refit: () => restty.requestLayoutSync(),
    dispose: () => {
      recolor.disconnect();
      restty.destroy();
    },
  };
}

/** Whether restty got a renderer, given up to 2 s to start. It is usually there within a few ms:
 *  looked at every 5 ms, the terminal connects as soon as it can draw (steps of 50 ms held every
 *  opening back by about 50 ms). Counted in steps, not time: a background tab's slowed timers
 *  must not make a terminal give up on the GPU. */
async function drawing(restty: { getBackend: () => string }): Promise<boolean> {
  for (let waited = 0; waited < 2000; waited += 5) {
    if (restty.getBackend() !== "none") return true;
    await new Promise((ok) => setTimeout(ok, 5));
  }
  return false;
}
