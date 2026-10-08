/* Why a machine failed, what to do, when each step happened, and the evidence. */
import { api } from "../../api.js";
import { h } from "../../dom.js";
import { toast } from "../../ui/toast.js";

const time = at => new Date(at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
const step = s => (s < 60 ? `+${Math.round(s)}s` : `+${Math.floor(s / 60)}m ${Math.round(s % 60)}s`);

/** The log the hint points at is the one worth opening first. */
function openFirst(d) {
  const hint = (d.hint || "").toLowerCase();
  const wanted = hint.includes("setup.sh") ? "share/setup.log"
    : hint.includes("console") ? "console.log"
    : hint.includes("claude's errors") ? "share/claude.err"
    : "share/job.log";
  return d.logs.some(l => l.file === wanted) ? wanted : d.logs[0]?.file;
}

/** The whole report as plain text, for a bug report or a message. */
export function diagnosticsText(id, d) {
  const lines = [`agentvm diagnostics — machine ${id}`, "", `Why: ${d.summary}`];
  if (d.hint) lines.push(`What to do: ${d.hint}`);
  lines.push("", "Timeline:", ...d.timeline.map(s => `  ${new Date(s.at * 1000).toISOString()}  ${s.state}`));
  for (const l of d.logs) lines.push("", `--- ${l.name} (${l.file}) ---`, l.tail);
  if (d.server_log.length) lines.push("", "--- Server log ---", ...d.server_log);
  return lines.join("\n");
}

export class DiagnosticsPanel {
  constructor(id) {
    this.id = id;
    this.body = h("div", { class: "diag-body" });
    const copy = h("button", { class: "btn small", type: "button", onclick: () => this.copy() }, "Copy diagnostics");
    const refresh = h("button", { class: "btn ghost small", type: "button", onclick: () => this.load() }, "Refresh");
    this.el = h("section", { class: "diagnostics", "aria-label": "Diagnostics" },
      h("header", {}, h("h2", {}, "Diagnostics"), h("div", { class: "row-actions" }, refresh, copy)), this.body);
    this.el.hidden = true;
  }

  async load() {
    const r = await api(`/api/tasks/${this.id}/diagnostics`);
    if (!r.ok) { this.body.replaceChildren(h("p", { class: "hint" }, r.data?.error || "Diagnostics are not available.")); return; }
    this.data = r.data;
    this.render(r.data);
  }

  render(d) {
    const first = openFirst(d);
    const timeline = h("ol", { class: "diag-timeline" }, d.timeline.map((s, i) =>
      h("li", { class: `st-${s.state}` }, h("span", { class: "when" }, time(s.at)), h("b", {}, s.state.replace("_", " ")),
        i > 0 && h("span", { class: "took" }, step(s.at - d.timeline[i - 1].at)))));
    const block = (title, text, open) => h("details", { open }, h("summary", {}, title), h("pre", {}, text || "(empty)"));
    // replaceChildren would print a `false` as text: conditional parts are left out instead.
    this.body.replaceChildren(...[
      h("div", { class: "diag-why" }, h("p", { class: "summary" }, d.summary), d.hint && h("p", { class: "diag-hint" }, d.hint)),
      d.timeline.length > 0 && timeline,
      ...d.logs.map(l => block(`${l.name} · ${l.file}`, l.tail, l.file === first)),
      d.server_log.length > 0 && block("Server log", d.server_log.join("\n"), false),
      d.logs.length === 0 && h("p", { class: "hint" }, "Its logs are gone (deleted with the closed VMs' logs)."),
    ].filter(Boolean));
  }

  async copy() {
    if (!this.data) await this.load();
    try { await navigator.clipboard.writeText(diagnosticsText(this.id, this.data)); toast("Diagnostics copied", "ok"); }
    catch { toast("Could not copy: select the text instead", "err"); }
  }

  show(open = true) { this.el.hidden = !open; if (open) this.load(); }
  toggle() { this.show(this.el.hidden); }
}
