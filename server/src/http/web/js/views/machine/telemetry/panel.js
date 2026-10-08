/* The telemetry panel next to a machine: live or historical charts, fresh or said to be stale,
   its processes, Claude's usage and the VM's facts. */
import { api } from "../../../api.js";
import { h } from "../../../dom.js";
import { isEnded } from "../../../task.js";
import { charts } from "./charts.js";
import { facts, processes, usage } from "./facts.js";

const RANGES = [["5m", "5 min"], ["1h", "1 hour"], ["all", "Whole life"]];
const REFRESH_MS = { "5m": 2000, "1h": 15000, all: 30000 };
/** Numbers older than this are shown as stale, not as live. */
const STALE_S = 5;

export class TelemetryPanel {
  constructor(id) {
    this.id = id;
    this.range = "5m";
    this.points = [];
    this.seg = h("div", { class: "seg range" }, RANGES.map(([key, label]) =>
      h("button", { type: "button", "aria-pressed": String(key === this.range), onclick: () => this.setRange(key) }, label)));
    this.el = h("div", { class: "telemetry" });
    this.fetch();
    this.timer = setInterval(() => this.fetch(), REFRESH_MS[this.range]);
  }

  setRange(key) {
    this.range = key;
    for (const b of this.seg.children) b.setAttribute("aria-pressed", String(b.textContent === RANGES.find(r => r[0] === key)[1]));
    clearInterval(this.timer);
    this.timer = setInterval(() => this.fetch(), REFRESH_MS[key]);
    this.fetch();
  }

  async fetch() {
    const r = await api(`/api/tasks/${this.id}/telemetry?range=${this.range}`);
    if (r.ok) { this.points = r.data.points; if (this.task) this.render(); }
  }

  /** Called on every refresh of the task list. */
  update(t, label) {
    // A closed VM has no live numbers: its whole life is what is left to show.
    if (isEnded(t) && !this.task && this.range === "5m") this.setRange("all");
    this.task = t;
    this.label = label;
    this.render();
  }

  render() {
    const t = this.task, ended = isEnded(t), m = ended ? null : t.metrics;
    const stale = !ended && t.metrics_age_s != null && t.metrics_age_s > STALE_S;
    this.el.replaceChildren(...[
      ended && h("p", { class: "closed-note" }, "This VM is closed: below is its history."),
      stale && h("p", { class: "stale" }, `No new numbers for ${Math.round(t.metrics_age_s)} s: the VM may be very busy or stuck. Showing the last ones.`),
      this.seg,
      ...charts(this.points, t),
      m && processes(m),
      t.usage && usage(t.usage),
      facts(t, this.label),
    ].filter(Boolean));
  }

  destroy() { clearInterval(this.timer); }
}
