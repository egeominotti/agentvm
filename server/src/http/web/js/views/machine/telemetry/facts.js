/* The telemetry panel's text: Claude's usage, the VM's busiest processes, and its facts. */
import { h } from "../../../dom.js";
import { gb, money, repoName, tokens } from "../../../format.js";
import { modelLabel } from "../../../models.js";
import { age } from "../../../task.js";

export function usage(u) {
  return h("div", { class: "stat" }, h("h3", {}, "Claude"),
    h("div", { class: "big" }, money(u.cost_usd), h("small", {}, "at API prices")),
    h("dl", { class: "kv" },
      h("dt", {}, "Tokens in"), h("dd", {}, tokens(u.input_tokens)),
      h("dt", {}, "Tokens out"), h("dd", {}, tokens(u.output_tokens)),
      h("dt", {}, "Lines changed"), h("dd", {}, h("span", { class: "plus" }, `+${u.lines_added}`), " ", h("span", { class: "minus" }, `−${u.lines_removed}`)),
      u.context_pct != null && h("dt", {}, "Context used"), u.context_pct != null && h("dd", {}, `${Math.round(u.context_pct)}%`)));
}

/** The processes using the most CPU and the most memory ("—" when the VM does not report it). */
export function processes(m) {
  const list = (title, rows, value) => h("div", {}, h("h3", {}, title), rows == null
    ? h("div", { class: "procs" }, h("span", {}, "—"))
    : rows.length ? h("div", { class: "procs" }, rows.map(p => h("div", {}, h("span", {}, p.name), h("span", {}, value(p)))))
      : h("div", { class: "procs" }, h("span", {}, "Idle")));
  return h("div", { class: "proc-lists" },
    list("Busiest (CPU)", m.top, p => `${p.cpu_pct.toFixed(0)}% · ${gb(p.mem_mb)}`),
    list("Largest (memory)", m.top_mem?.length ? m.top_mem : null, p => gb(p.mem_mb)));
}

export function facts(t, label) {
  return h("dl", { class: "kv" },
    h("dt", {}, "State"), h("dd", {}, label),
    h("dt", {}, "Model"), h("dd", {}, modelLabel(t.model)),
    h("dt", {}, "Claude Code"), h("dd", {}, t.claude_version ?? "image version"),
    h("dt", {}, "Resources"), h("dd", {}, `${t.cpus} vCPUs, ${gb(t.memory_mb)}`),
    h("dt", {}, "Repository"), h("dd", { title: t.repo }, repoName(t.repo)),
    h("dt", {}, "Branch"), h("dd", { title: t.branch }, t.branch),
    h("dt", {}, "From commit"), h("dd", {}, t.base_sha.slice(0, 10)),
    h("dt", {}, "Age"), h("dd", {}, age(t)));
}
