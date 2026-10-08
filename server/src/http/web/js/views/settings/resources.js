/* Settings › Resources: how many VMs at once, and the vCPUs and memory of each. */
import { h } from "../../dom.js";
import { gb } from "../../format.js";
import { state } from "../../state.js";

/** Memory kept for macOS, never given to VMs. */
const RESERVE_MB = 8192;

export function resourcesSection({ draft: s, set }) {
  const lim = state.settings.limits;
  const memChoices = [1024, 2048, 4096, 6144, 8192, 12288, 16384].filter(v => v <= lim.ram_mb - RESERVE_MB);
  const vmsOut = h("output", { class: "big-value" });
  const cpuOut = h("output", { class: "big-value" });
  const vmsInput = h("input", { type: "range", min: "1", max: String(Math.min(64, Math.max(lim.cpus * 2, 16))), value: String(s.max_vms), oninput: e => set("max_vms", Number(e.target.value)) });
  const cpuInput = h("input", { type: "range", min: "1", max: String(lim.cpus), value: String(s.cpus), oninput: e => set("cpus", Number(e.target.value)) });
  const memSeg = h("div", { class: "seg wide", role: "radiogroup", "aria-label": "Memory per VM" },
    memChoices.map(v => h("button", { type: "button", role: "radio", "data-value": String(v), onclick: () => set("memory_mb", v) }, gb(v))));
  const budget = h("div", { class: "budget" });
  const error = h("p", { class: "field-error", role: "alert" });
  const block = h("div", { class: "res" },
    h("div", { class: "slider" }, h("div", { class: "slider-head" }, h("label", {}, "VMs at the same time"), vmsOut), vmsInput,
      h("p", { class: "hint" }, "Extra launches wait in a queue. Takes effect at once.")),
    h("div", { class: "slider" }, h("div", { class: "slider-head" }, h("label", {}, "vCPUs per VM"), cpuOut), cpuInput,
      h("p", { class: "hint" }, `This Mac has ${lim.cpus} cores. vCPUs are shared, so the total can exceed them.`)),
    h("div", { class: "slider" }, h("div", { class: "slider-head" }, h("label", {}, "Memory per VM")), memSeg,
      h("p", { class: "hint" }, "Reserved for each running VM. New VMs use the new size.")));

  /** The values, and the memory budget they add up to. */
  function paint() {
    const total = state.settings.limits.ram_mb, reserve = RESERVE_MB, used = s.max_vms * s.memory_mb, fit = Math.max(1, Math.floor((total - reserve) / s.memory_mb));
    vmsOut.textContent = String(s.max_vms);
    cpuOut.textContent = String(s.cpus);
    for (const b of memSeg.children) b.setAttribute("aria-checked", String(Number(b.dataset.value) === s.memory_mb));
    const over = used > total - reserve;
    budget.className = `budget${over ? " over" : ""}`;
    budget.replaceChildren(
      h("div", { class: "budget-bar" },
        h("i", { class: "vms", style: `width:${Math.min(100, (100 * used) / total)}%` }),
        h("i", { class: "reserve", style: `width:${(100 * reserve) / total}%` })),
      h("div", { class: "budget-legend" },
        h("span", {}, h("b", {}, `${s.max_vms} × ${gb(s.memory_mb)} = ${gb(used)}`), ` for VMs at full load`),
        h("span", {}, `${gb(reserve)} kept for macOS`),
        h("span", { class: "fit" }, over ? `Over budget: at most ${fit} VMs of ${gb(s.memory_mb)} fit without swapping` : `Fits: up to ${fit} VMs of ${gb(s.memory_mb)}`)));
  }

  paint();
  return {
    body: [h("p", { class: "lede" }, `Resources of this Mac given to the VMs: ${lim.cpus} cores, ${gb(lim.ram_mb)} of memory.`), block, budget, error],
    changed: paint,
    showError: text => (error.textContent = text),
  };
}
