// The WebSocket to one tmux session of a VM: binary frames are keys and output, a JSON text
// frame is a size. It reconnects while the VM runs, keeps the keys typed while connecting
// (sent once open, not lost), and leaves out sizes of a hidden box (they would shrink the VM's
// window for every other client).

export type PtySocketOptions = {
  id: string;
  session: string;
  /** A read-only client that does not resize the window (wall previews). */
  view?: boolean;
  /** Whether to reconnect after the socket closes (the VM still runs). */
  live: () => boolean;
  /** Output gathered and handed on at most this often (previews: a wall of many stays light). */
  batchMs?: number;
};

export class PtySocket {
  private ws: WebSocket | null = null;
  private retry: ReturnType<typeof setTimeout> | undefined;
  private closed = false;
  private pending: Uint8Array[] = [];
  private size = { cols: 100, rows: 30 };
  private readonly enc = new TextEncoder();
  private listeners: ((bytes: Uint8Array) => void)[] = [];
  private openListeners: (() => void)[] = [];
  private batch: Uint8Array[] = [];
  private flush: ReturnType<typeof setTimeout> | undefined;
  private paused = false;

  constructor(private readonly opts: PtySocketOptions) {}

  onOutput(listener: (bytes: Uint8Array) => void) {
    this.listeners.push(listener);
  }

  /** Each time a connection opens (the first, and after every reconnection). */
  onOpen(listener: () => void) {
    this.openListeners.push(listener);
  }

  connect(cols: number, rows: number) {
    if (cols >= 20 && rows >= 5) this.size = { cols, rows };
    if (this.closed || this.paused || this.ws) return;
    const { id, session, view } = this.opts;
    const url = `ws://${location.host}/api/tasks/${id}/pty?session=${session}&cols=${this.size.cols}&rows=${this.size.rows}${view ? "&view=true" : ""}`;
    const sock = new WebSocket(url);
    sock.binaryType = "arraybuffer";
    sock.onopen = () => {
      // Sizes sent while connecting are lost: the real one goes once the socket is open.
      this.sendSize();
      for (const p of this.pending) sock.send(p);
      this.pending = [];
      for (const l of this.openListeners) l();
    };
    sock.onmessage = (e) => this.output(new Uint8Array(e.data as ArrayBuffer));
    sock.onclose = () => {
      if (this.ws !== sock) return;
      this.ws = null;
      if (!this.closed && !this.paused && this.opts.live()) this.retry = setTimeout(() => this.connect(0, 0), 900);
    };
    this.ws = sock;
  }

  /** Keys (or a paste) for the session; kept until the socket is open. */
  send(text: string) {
    const bytes = this.enc.encode(text);
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(bytes);
    else if (this.pending.length < 256) this.pending.push(bytes);
  }

  resize(cols: number, rows: number) {
    // A hidden or collapsed box measures a few cells: never shrink the VM's window to that.
    if (cols < 20 || rows < 5) return;
    this.size = { cols, rows };
    this.sendSize();
  }

  isOpen() {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  /** Disconnects until `resume` (a page in the background): tmux redraws all on reattach. */
  pause() {
    this.paused = true;
    clearTimeout(this.retry);
    const sock = this.ws;
    this.ws = null;
    sock?.close();
  }

  resume() {
    this.paused = false;
    this.connect(0, 0);
  }

  close() {
    this.closed = true;
    clearTimeout(this.retry);
    clearTimeout(this.flush);
    this.ws?.close();
    this.ws = null;
  }

  private output(bytes: Uint8Array) {
    if (!this.opts.batchMs) {
      for (const l of this.listeners) l(bytes);
      return;
    }
    this.batch.push(bytes);
    this.flush ??= setTimeout(() => {
      this.flush = undefined;
      const all = new Uint8Array(this.batch.reduce((n, b) => n + b.length, 0));
      let at = 0;
      for (const b of this.batch) {
        all.set(b, at);
        at += b.length;
      }
      this.batch = [];
      for (const l of this.listeners) l(all);
    }, this.opts.batchMs);
  }

  private sendSize() {
    if (this.ws?.readyState === WebSocket.OPEN && !this.opts.view) this.ws.send(JSON.stringify(this.size));
  }
}
