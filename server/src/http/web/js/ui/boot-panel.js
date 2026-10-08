/* The boot sequence of a machine, shown until its terminal is ready. */
import { h } from "../dom.js";
import { age } from "../task.js";

/** Turns the task's boot log into steps: real events with real timings. */
function bootSteps(t) {
  const host = {}, guest = [];
  for (const line of t.boot_log || []) {
    let m = line.match(/^host: (repository (?:packed|shared)|disk ready) in (\d+) ms/);
    if (m) { host[m[1].startsWith("repository") ? "repository" : m[1]] = `${m[2]} ms`; continue; }
    m = line.match(/^\[([\d.]+)s\] (.*)$/);
    if (m) guest.push({ at: Number(m[1]), text: m[2] });
  }
  const seen = re => guest.find(g => re.test(g.text));
  const secs = g => (g ? `${g.at.toFixed(1)} s` : "");
  const s = t.status.state;
  const steps = [
    { label: "VM slot reserved", done: s !== "queued", detail: s === "queued" ? "waiting for a free slot" : "" },
    { label: "Disk cloned from the image", done: !!host["disk ready"], detail: host["disk ready"] || "" },
    { label: "Debian booted", done: !!seen(/^job start/), detail: secs(seen(/^job start/)) },
    { label: "Repository checked out", done: !!seen(/^repo ready/), detail: secs(seen(/^repo ready/)) },
    { label: "Network up", done: !!seen(/^network ready/), detail: secs(seen(/^network ready/)) },
  ];
  const setup = seen(/^running \.agentvm\/setup\.sh/);
  if (setup) {
    const end = seen(/^setup (done|failed)/);
    steps.push({ label: "Repository setup (.agentvm/setup.sh)", done: !!end, failed: end && /failed/.test(end.text), detail: end ? (/failed/.test(end.text) ? "failed, see setup.log" : secs(end)) : "running" });
  }
  const install = seen(/^installing Claude Code/);
  if (install) steps.push({ label: install.text.replace(/^installing/, "Installing"), done: !!seen(/^Claude Code \d/), detail: secs(seen(/^Claude Code \d/)) });
  const readyEvt = seen(t.interactive ? /^terminal ready/ : /^network ready/);
  steps.push({ label: t.interactive ? "Claude Code ready" : "Agent started", done: t.ready || !!readyEvt, detail: secs(readyEvt) });
  const current = steps.findIndex(x => !x.done);
  return { steps, current: current < 0 ? steps.length : current };
}

const BOOT_MARK = `<svg viewBox="0 0 64 64" class="boot-mark" aria-hidden="true">
  <g class="brackets" stroke-linecap="square" fill="none">
    <path d="M6 20V6h14"/><path d="M44 6h14v14"/><path d="M58 44v14H44"/><path d="M20 58H6V44"/>
  </g>
  <rect class="core" x="22" y="22" width="12" height="20"/>
  <rect class="scan" x="8" y="8" width="48" height="2"/>
</svg>`;

/** Boot screen: built once (the animation never restarts), then `update(t)` every tick. */
export function bootPanel(compact = false) {
  const mark = h("div", { class: `boot-mark-wrap${compact ? " small" : ""}` });
  mark.innerHTML = BOOT_MARK;
  const kicker = h("span", { class: "boot-kicker" });
  const now = h("b", { class: "boot-now" });
  const clock = h("span", { class: "boot-clock" });
  const fill = h("i");
  const list = h("ol", { class: "boot-steps" });
  const root = compact
    ? h("div", { class: "boot compact" }, mark, now, h("div", { class: "boot-bar" }, fill), clock)
    : h("div", { class: "boot" }, mark, h("div", { class: "boot-head" }, kicker, now, clock), h("div", { class: "boot-bar" }, fill), list);
  root.update = t => {
    const { steps, current } = bootSteps(t);
    kicker.textContent = t.status.state === "queued" ? "Queued" : "Sealing your machine";
    now.textContent = current < steps.length ? `${steps[current].label}…` : "Opening the terminal…";
    clock.textContent = compact ? `${current} of ${steps.length}, ${age(t)}` : age(t);
    fill.style.width = `${(100 * current) / steps.length}%`;
    if (!compact) {
      list.replaceChildren(...steps.map((x, k) =>
        h("li", { class: x.failed ? "failed" : x.done ? "done" : k === current ? "now" : "" },
          h("span", { class: "tick" }), h("span", { class: "what" }, x.label), h("span", { class: "when" }, x.detail))));
    }
  };
  return root;
}
