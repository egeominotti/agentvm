/* Claude's history in a machine: usage over time and the whole conversation, kept after the
   machine is closed. */
import { api } from "../../../api.js";
import { h } from "../../../dom.js";
import { money, tokens } from "../../../format.js";
import { lineChart } from "../../../ui/line-chart.js";
import { entry, turns } from "./entries.js";

const BLUE = "#3987e5";
const PAGE = 500;

export class ConversationPanel {
  constructor(id) {
    this.id = id;
    this.entries = [];
    this.next = 0;
    this.samples = [];
    this.list = h("div", { class: "conversation" });
    this.head = h("div", { class: "conv-head" });
    this.charts = h("div", { class: "conv-charts" });
    this.el = h("section", { class: "diagnostics claude-history", "aria-label": "Claude's history" },
      h("header", {}, h("h2", {}, "Claude's history"), this.head), this.charts, this.list);
    this.el.hidden = true;
  }

  /** Reads what is new since the last call: the next pages of the conversation, all the usage. */
  async load() {
    for (let more = true; more;) {
      const r = await api(`/api/tasks/${this.id}/claude?after=${this.next}`);
      if (!r.ok) break;
      const lines = r.data.next - this.next;
      this.entries.push(...r.data.entries);
      this.list.append(...r.data.entries.map(entry).filter(Boolean));
      this.next = r.data.next;
      more = lines >= PAGE;
    }
    const u = await api(`/api/tasks/${this.id}/claude/usage`);
    if (u.ok) this.samples = u.data.samples;
    this.renderUsage();
    if (!this.entries.length) this.list.replaceChildren(h("p", { class: "hint" }, "No conversation yet."));
  }

  renderUsage() {
    const last = this.samples[this.samples.length - 1];
    this.head.replaceChildren(last ? h("span", { class: "hint" },
      `${money(last.cost_usd)} · ${tokens(last.input_tokens)} in · ${tokens(last.output_tokens)} out`,
      last.context_pct != null ? ` · context ${Math.round(last.context_pct)}%` : "") : "");
    const per = turns(this.entries), times = this.samples.map(s => s.at);
    this.charts.replaceChildren(
      lineChart({ title: "Cost (API prices)", times, format: money, series: [{ name: "Cost", color: BLUE, values: this.samples.map(s => s.cost_usd) }] }),
      lineChart({ title: "Output tokens per call", times: per.map(t => t.at), format: tokens, series: [{ name: "Output", color: BLUE, values: per.map(t => t.output_tokens) }] }),
      lineChart({ title: "Context used", times, max: 100, format: v => `${v.toFixed(0)}%`, empty: "Not reported (automatic tasks)",
        series: this.samples.some(s => s.context_pct != null) ? [{ name: "Context", color: BLUE, values: this.samples.map(s => s.context_pct ?? null) }] : [] }));
  }

  /** `live`: the VM is running, so its conversation keeps growing (read again every 5 s). */
  show(open = true, live = true) {
    this.el.hidden = !open;
    clearInterval(this.timer);
    if (!open) return;
    this.load();
    if (live) this.timer = setInterval(() => this.load(), 5000);
  }

  toggle(live) { this.show(this.el.hidden, live); }
  destroy() { clearInterval(this.timer); }
}
