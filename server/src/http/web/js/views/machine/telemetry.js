/* The telemetry panel next to a machine: Claude's usage, the VM's load, and its facts. */
import { h } from "../../dom.js";
import { duration, gb, money, rate, repoName, tokens } from "../../format.js";
import { modelLabel } from "../../models.js";
import { age, isEnded } from "../../task.js";
import { sparkline } from "../../ui/sparkline.js";

function load(t, m) {
  return [
    h("div", { class: "stat" }, h("h3", {}, "CPU"), h("div", { class: "big" }, `${m.cpu_pct.toFixed(0)}%`, h("small", {}, `of ${m.cpus} vCPUs`)), sparkline(t.cpu_history, { width: 250, height: 40, fluid: true })),
    h("div", { class: "stat" }, h("h3", {}, "Memory"), h("div", { class: "big" }, gb(m.mem_used_mb), h("small", {}, `of ${gb(m.mem_total_mb)}`)), sparkline(t.mem_history, { width: 250, height: 40, color: "var(--ink-3)", fluid: true })),
    h("div", { class: "stat" }, h("h3", {}, "Disk"), h("div", {}, `${gb(m.disk_used_mb)} of ${gb(m.disk_total_mb)}`),
      h("div", { class: "bar-line" }, h("i", { style: `width:${(100 * m.disk_used_mb) / Math.max(m.disk_total_mb, 1)}%` }))),
    h("div", {}, h("h3", {}, "Busiest processes"), m.top.length
      ? h("div", { class: "procs" }, m.top.map(p => h("div", {}, h("span", {}, p.name), h("span", {}, `${p.cpu_pct.toFixed(0)}%`), h("span", {}, gb(p.mem_mb)))))
      : h("div", { class: "procs" }, h("span", {}, "Idle"))),
    h("dl", { class: "kv" },
      h("dt", {}, "Network in"), h("dd", {}, rate(m.net_rx_bps)),
      h("dt", {}, "Network out"), h("dd", {}, rate(m.net_tx_bps)),
      h("dt", {}, "Load"), h("dd", {}, m.load1.toFixed(2)),
      h("dt", {}, "Processes"), h("dd", {}, String(m.procs)),
      h("dt", {}, "VM uptime"), h("dd", {}, duration(m.uptime_s))),
  ];
}

function usage(u) {
  return h("div", { class: "stat" }, h("h3", {}, "Claude"),
    h("div", { class: "big" }, money(u.cost_usd), h("small", {}, "at API prices")),
    h("dl", { class: "kv" },
      h("dt", {}, "Tokens in"), h("dd", {}, tokens(u.input_tokens)),
      h("dt", {}, "Tokens out"), h("dd", {}, tokens(u.output_tokens)),
      h("dt", {}, "Lines changed"), h("dd", {}, h("span", { class: "plus" }, `+${u.lines_added}`), " ", h("span", { class: "minus" }, `−${u.lines_removed}`)),
      u.context_pct != null && h("dt", {}, "Context used"), u.context_pct != null && h("dd", {}, `${Math.round(u.context_pct)}%`)));
}

function facts(t, label) {
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

/** The panel's children for task `t`, whose state reads `label`. */
export function telemetry(t, label) {
  const ended = isEnded(t);
  const m = ended ? null : t.metrics;
  return [
    ended && h("p", { class: "closed-note" }, "This VM is closed: its disk is gone, the work is on the branch."),
    t.usage && usage(t.usage),
    ...(m ? load(t, m) : []),
    facts(t, label),
  ].filter(Boolean);
}
