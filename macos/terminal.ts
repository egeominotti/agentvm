// Only the terminal renderer is shared with the dashboard; navigation and controls are native.
import { openView, type TerminalView } from "../web/src/features/machine/terminal/engine";
import type { PtySocket } from "../web/src/features/machine/terminal/socket";

const post = (message: Record<string, unknown>) => {
  (window as any).webkit.messageHandlers.nativeTerminal.postMessage(message);
};

class NativeSocket {
  private listeners: ((bytes: Uint8Array) => void)[] = [];
  private opens: (() => void)[] = [];
  private connected = false;
  onOutput(listener: (bytes: Uint8Array) => void) { this.listeners.push(listener); }
  onOpen(listener: () => void) { this.opens.push(listener); }
  connect(cols: number, rows: number) { post({ type: "connect", cols, rows }); }
  send(text: string) { post({ type: "input", text }); }
  resize(cols: number, rows: number) { post({ type: "resize", cols, rows }); }
  isOpen() { return this.connected; }
  output(data: string) {
    const bytes = Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
    for (const listener of this.listeners) listener(bytes);
  }
  state(connected: boolean) {
    this.connected = connected;
    if (connected) for (const open of this.opens) open();
  }
}

Object.defineProperty(navigator, "clipboard", { value: {
  writeText: async (text: string) => post({ type: "copy", text }),
}});
document.addEventListener("keydown", (event) => {
  if (event.metaKey && event.key.toLowerCase() === "v") {
    event.preventDefault();
    event.stopImmediatePropagation();
    post({ type: "paste" });
  }
}, true);

const socket = new NativeSocket();
let view: TerminalView | null = null;
const box = document.getElementById("terminal")!;
(window as any).nativeTerminal = {
  output: (data: string) => socket.output(data),
  state: (connected: boolean) => socket.state(connected),
  paste: (text: string) => view?.paste(text),
  theme: (theme: string) => { document.documentElement.dataset.theme = theme; },
  focus: () => view?.focus(),
};
const initialTheme = new URLSearchParams(location.search).get("theme");
document.documentElement.dataset.theme = initialTheme === "light" || initialTheme === "dark"
  ? initialTheme
  : matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
// xterm installs its OSC52 parser only with this callback. The native bridge reports writes.
await openView(box, {
  socket: socket as unknown as PtySocket,
  onCopied: (ok) => { if (!ok) post({ type: "error", message: "The terminal could not send its selection to the clipboard." }); },
}).then((created) => {
  view = created;
  (window as any).nativeTerminal.ready = true;
  (window as any).nativeTerminal.engine = view.engine;
  post({ type: "ready", engine: view.engine });
}).catch((error) => post({ type: "error", message: error.message }));
