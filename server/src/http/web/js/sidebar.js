/* The sidebar: the running machines, alerts and this Mac's load. */
import { $, h } from "./dom.js";
import { gb, money } from "./format.js";
import { state } from "./state.js";
import { age, isEnded, isWaiting, kind, titleOf } from "./task.js";

function gauge(label, value, pct, hot) {
  return h("div", { class: `gauge${hot ? " hot" : ""}` },
    h("div", { class: "row" }, h("span", {}, label), h("b", {}, value)),
    h("div", { class: "track" }, h("i", { style: `width:${Math.min(100, Math.max(0, pct))}%` })));
}

/** This Mac, at the bottom of the sidebar, and the alerts at its top. */
export function renderHost() {
  const s = state.status;
  const box = $("#host");
  if (!s) return;
  const live = state.tasks.filter(t => t.metrics && t.status.state === "running");
  const cpu = live.length ? live.reduce((n, t) => n + t.metrics.cpu_pct * t.metrics.cpus, 0) / s.host.cpus : 0;
  const kids = [
    gauge("VMs", `${s.running} / ${s.concurrency}`, (100 * s.running) / s.concurrency, s.running >= s.concurrency),
    gauge("Memory reserved", `${gb(s.ram_committed_mb)} / ${gb(s.host.ram_mb)}`, (100 * s.ram_committed_mb) / s.host.ram_mb, s.ram_committed_mb > s.host.ram_mb * 0.85),
    gauge("CPU in VMs", `${cpu.toFixed(0)}%`, cpu, cpu > 85),
  ];
  const spent = state.tasks.reduce((n, t) => n + (t.usage?.cost_usd ?? 0), 0);
  kids.push(h("div", { class: "gauge spend", title: "What these agents would cost at API prices; with a subscription it counts against its limits" },
    h("div", { class: "row" }, h("span", {}, "Claude usage"), h("b", {}, money(spent)))));
  box.replaceChildren(...kids);
  const alerts = [];
  if (!s.token) alerts.push(h("button", { class: "pill-warn", type: "button", onclick: () => (location.hash = "#/settings") }, "Claude token missing"));
  if (!s.golden) alerts.push(h("button", { class: "pill-warn", type: "button", onclick: () => (location.hash = "#/settings") }, "VM image missing"));
  $("#alerts").replaceChildren(...alerts);
}

/** Every machine in the sidebar, like a list of issues. Patched in place: a link replaced
 * while the mouse button is down would swallow the click. */
const sideItems = new Map();
export function renderSide() {
  const focused = location.hash.match(/^#\/vm\/(.+)$/)?.[1];
  const live = state.tasks.filter(t => !isEnded(t)).length;
  const count = live ? String(live) : "";
  if ($("#count-machines").textContent !== count) $("#count-machines").textContent = count;
  // Running machines only: finished ones stay on the wall until cleared.
  const shown = state.tasks.filter(t => !isEnded(t) || t.id === focused);
  const box = $("#side-machines");
  for (const [id, el] of sideItems) if (!shown.some(t => t.id === id)) { el.remove(); sideItems.delete(id); }
  shown.forEach((t, i) => {
    const [cls, label] = kind(t);
    let el = sideItems.get(t.id);
    if (!el) {
      el = h("a", { href: `#/vm/${t.id}` }, h("span", { class: "dot" }), h("span", { class: "t" }), h("span", { class: "m" }));
      sideItems.set(t.id, el);
    }
    const set = (node, text) => { if (node.textContent !== text) node.textContent = text; };
    const className = `side-vm ${cls}`;
    if (el.className !== className) el.className = className;
    el.setAttribute("aria-current", String(t.id === focused));
    el.title = `${titleOf(t)}\n${label}`;
    set(el.children[1], titleOf(t));
    set(el.children[2], isWaiting(t) ? "waiting" : age(t));
    if (box.children[i] !== el) box.insertBefore(el, box.children[i] ?? null);
  });
  const empty = box.querySelector(".side-empty");
  if (!shown.length && !empty) box.append(h("p", { class: "side-empty" }, "No running machines"));
  if (shown.length && empty) empty.remove();
}
