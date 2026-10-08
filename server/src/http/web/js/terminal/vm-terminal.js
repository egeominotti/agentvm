/* A terminal attached to a tmux session of a VM, streamed over a WebSocket. */
import { h } from "../dom.js";
import { byId } from "../state.js";
import { copySelectionsToClipboard } from "./clipboard.js";
import { openXterm } from "./xterm.js";

/** Full-size terminals fit their element; `fixed` ones (previews) keep the given size and are scaled down. */
export class VmTerminal {
  constructor(id, session, { fontSize = 13, webgl = true, readOnly = false, fixed = null } = {}) {
    this.id = id;
    this.session = session;
    this.fixed = fixed;
    this.el = h("div", { class: fixed ? "term fixed" : "term" });
    const { xterm, fitter } = openXterm(this.el, { fontSize, readOnly, webgl });
    this.xterm = xterm;
    this.fitter = fitter;
    copySelectionsToClipboard(xterm, readOnly);
    const enc = new TextEncoder();
    if (!readOnly) xterm.onData(d => { if (this.ws?.readyState === 1) this.ws.send(enc.encode(d)); });
    xterm.onResize(({ cols, rows }) => { if (this.ws?.readyState === 1) this.ws.send(JSON.stringify({ cols, rows })); });
    this.resizer = new ResizeObserver(() => this.fitSoon());
    this.resizer.observe(fixed ? document.body : this.el);
  }

  fitSoon() {
    if (this.pending) return;
    this.pending = true;
    requestAnimationFrame(() => { this.pending = false; this.fit(); });
  }

  fit() {
    if (!this.el.isConnected || !this.el.offsetWidth) return;
    if (this.fixed) return this.scaleToParent();
    const d = this.fitter.proposeDimensions();
    if (d && (d.cols !== this.xterm.cols || d.rows !== this.xterm.rows)) this.xterm.resize(d.cols, d.rows);
  }

  /** Previews keep the session's real size and are scaled down to fit their cell. */
  scaleToParent() {
    const box = this.el.parentElement;
    const screen = this.el.querySelector(".xterm-screen");
    if (!box || !screen || !screen.offsetWidth) return;
    const k = Math.min(box.clientWidth / screen.offsetWidth, box.clientHeight / screen.offsetHeight, 1);
    this.el.style.transform = `scale(${k})`;
  }

  connect() {
    if (this.ws || this.disposed) return;
    if (this.fixed) this.xterm.resize(this.fixed.cols, this.fixed.rows);
    this.fit();
    if (this.xterm.cols < 20) this.xterm.resize(100, 30);
    const { cols, rows } = this.xterm;
    const view = this.fixed ? "&view=true" : "";
    const ws = new WebSocket(`ws://${location.host}/api/tasks/${this.id}/pty?session=${this.session}&cols=${cols}&rows=${rows}${view}`);
    ws.binaryType = "arraybuffer";
    // Resizes sent while connecting are lost: send the real size once the socket is open.
    ws.onopen = () => { this.fit(); ws.send(JSON.stringify({ cols: this.xterm.cols, rows: this.xterm.rows })); };
    ws.onmessage = this.fixed ? e => this.writeBatched(e.data) : e => this.xterm.write(new Uint8Array(e.data));
    ws.onclose = () => {
      if (this.ws !== ws) return;
      this.ws = null;
      if (!this.disposed && byId(this.id)?.status.state === "running") setTimeout(() => this.connect(), 900);
    };
    this.ws = ws;
  }

  /** Previews: parse at most 4 times a second, whatever the VM prints (3-5x less browser CPU). */
  writeBatched(data) {
    (this.chunks ??= []).push(new Uint8Array(data));
    this.flushTimer ??= setTimeout(() => {
      this.flushTimer = null;
      const chunks = this.chunks;
      this.chunks = [];
      for (const c of chunks) this.xterm.write(c);
    }, 250);
  }

  /** Stops streaming (a preview nobody can see); `connect()` resumes and tmux redraws it all. */
  pause() {
    const ws = this.ws;
    this.ws = null;
    ws?.close();
  }

  dispose() {
    this.disposed = true;
    clearTimeout(this.flushTimer);
    this.resizer.disconnect();
    this.ws?.close();
    this.xterm.dispose();
    this.el.remove();
  }
}
