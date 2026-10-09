// Which engine draws a terminal: restty (Ghostty's core, WebGPU or WebGL2, faster on heavy output)
// when the browser can run it, xterm.js otherwise or if restty fails to start. A terminal is
// never left blank because of the engine.
import { monoFont } from "./font";
import type { PtySocket } from "./socket";

export type ViewOptions = {
  socket: PtySocket;
  /** Watch only: no keys, no resizing the VM's window (wall previews). */
  readOnly?: boolean;
  fontSize?: number;
  /** A fixed size instead of following the box (previews are scaled down instead). */
  fixed?: { cols: number; rows: number };
  /** Text selected in the terminal went to the clipboard (or the browser refused). */
  onCopied?: (ok: boolean) => void;
};

export type TerminalView = {
  engine: "restty" | "xterm";
  focus: () => void;
  /** Typed into the session, as if by the user. */
  paste: (text: string) => void;
  /** The box changed (shown again, resized): take its size. */
  refit: () => void;
  dispose: () => void;
};

/** restty needs WebGPU or WebGL2. */
function resttyCanRun(): boolean {
  if ("gpu" in navigator) return true;
  try {
    return !!document.createElement("canvas").getContext("webgl2");
  } catch {
    return false;
  }
}

export async function openView(el: HTMLElement, opts: ViewOptions): Promise<TerminalView> {
  // Each view draws in a box of its own, removed with it: a view that finishes opening after its
  // terminal was already closed and opened again (React remounts, a VM restarting) then disposes
  // only itself, never the canvas of the view that replaced it.
  const host = document.createElement("div");
  host.className = "term-host";
  el.append(host);
  const view = await open(host, opts);
  el.dataset.engine = host.dataset.engine;
  return {
    ...view,
    dispose: () => {
      view.dispose();
      host.remove();
    },
  };
}

async function open(host: HTMLElement, opts: ViewOptions): Promise<TerminalView> {
  if (resttyCanRun()) {
    try {
      const { openRestty } = await import("./restty-view");
      return await openRestty(host, opts);
    } catch (e) {
      console.warn("restty could not start; xterm.js draws this terminal", e);
      host.replaceChildren();
    }
  }
  const { openXtermView } = await import("./xterm-view");
  // xterm measures its cells once: the font must be there first.
  await monoFont();
  return openXtermView(host, opts);
}
