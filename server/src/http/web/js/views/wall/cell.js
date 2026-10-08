/* One machine on the wall: a live, read-only preview of its Claude session. */
import { h } from "../../dom.js";
import { gb, money, repoName, tokens } from "../../format.js";
import { age, isEnded, kind, titleOf } from "../../task.js";
import { VmTerminal } from "../../terminal/vm-terminal.js";
import { bootPanel } from "../../ui/boot-panel.js";
import { sparkline } from "../../ui/sparkline.js";

export class Cell {
  constructor(t) {
    this.id = t.id;
    this.state = h("span", { class: "state" });
    this.title = h("span", { class: "title" });
    this.body = h("div", { class: "cell-body", onclick: () => (location.hash = `#/vm/${t.id}`), title: "Open this machine" });
    this.cpu = h("span", { class: "metric" });
    this.mem = h("span", { class: "metric" });
    this.proc = h("span", { class: "proc" });
    this.root = h("article", { class: "cell" },
      h("div", { class: "cell-head" }, h("span", { class: "dot" }), this.title, this.state),
      this.body,
      h("div", { class: "cell-foot" }, this.cpu, this.mem, this.proc));
  }

  update(t) {
    const [cls, label] = kind(t);
    this.root.className = `cell ${cls}${isEnded(t) ? " ended-cell" : ""}`;
    this.title.textContent = titleOf(t);
    this.title.title = `${titleOf(t)}\n${t.repo}`;
    this.state.textContent = `${label}, ${age(t)}`;
    this.paintFoot(t);
    if (!isEnded(t) && t.interactive && !(t.status.state === "running" && t.ready)) this.showBoot(t);
    else if (t.status.state === "running" && t.interactive) this.showTerminal(t);
    else this.showEnded(t, label);
  }

  paintFoot(t) {
    const m = t.metrics;
    this.cpu.replaceChildren(h("b", {}, m ? `${m.cpu_pct.toFixed(0)}%` : "—"), "CPU", sparkline(t.cpu_history, { width: 70, height: 16 }));
    this.mem.replaceChildren(h("b", {}, m ? gb(m.mem_used_mb) : "—"), "RAM", sparkline(t.mem_history, { width: 70, height: 16, color: "var(--ink-3)" }));
    this.portChip ??= h("a", { class: "port-chip", target: "_blank", rel: "noopener", onclick: e => e.stopPropagation() });
    const p0 = t.ports?.find(p => p.kind === "http") ?? t.ports?.[0];
    this.portChip.hidden = !p0;
    if (p0) {
      this.portChip.href = p0.url ?? `http://localhost:${p0.host_port}`;
      this.portChip.title = p0.url ?? `localhost:${p0.host_port}`;
      this.portChip.textContent = `:${p0.port}${t.ports.length > 1 ? ` +${t.ports.length - 1}` : ""}`;
    }
    if (!this.portChip.isConnected) this.proc.before(this.portChip);
    const u = t.usage;
    this.proc.textContent = u?.output_tokens ? `${money(u.cost_usd)}  ${tokens(u.input_tokens + u.output_tokens)} tokens` : m?.top?.[0] ? `${m.top[0].name} ${m.top[0].cpu_pct.toFixed(0)}%` : repoName(t.repo);
  }

  showBoot(t) {
    this.term?.dispose();
    this.term = null;
    this.endedFor = null;
    if (!this.boot || !this.body.contains(this.boot)) { this.boot = bootPanel(true); this.body.replaceChildren(this.boot); }
    this.boot.update(t);
  }

  showTerminal(t) {
    if (!this.term) {
      this.body.replaceChildren();
      this.term = new VmTerminal(t.id, "claude", { fontSize: 12, webgl: false, readOnly: true, fixed: { cols: 160, rows: 48 } });
      this.body.append(this.term.el);
      this.resize ??= new ResizeObserver(() => this.term?.fitSoon());
      this.resize.observe(this.body);
      // Previews stream only while on screen and while the page is visible.
      this.seen ??= new IntersectionObserver(([e]) => { this.visible = e.isIntersecting; this.stream(); });
      this.seen.observe(this.root);
    }
    this.stream();
  }

  showEnded(t, label) {
    this.term?.dispose();
    this.term = null;
    const text = isEnded(t)
      ? [h("b", {}, label), h("span", {}, repoName(t.repo))]
      : [h("b", {}, label), h("span", {}, t.interactive ? "Debian boots in a few seconds, then Claude opens." : "Running without a terminal.")];
    if (this.endedFor !== label) { this.endedFor = label; this.body.replaceChildren(h("div", { class: "ended" }, ...text)); }
  }

  /** Streams the preview while it is on screen and the page is visible; pauses it otherwise. */
  stream() {
    if (!this.term) return;
    if (this.visible !== false && !document.hidden) this.term.connect();
    else this.term.pause();
  }

  destroy() { this.term?.dispose(); this.resize?.disconnect(); this.seen?.disconnect(); }
}
