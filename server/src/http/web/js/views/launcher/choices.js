/* New VM: the model, Claude Code version and resources of the machines to launch. */
import { api } from "../../api.js";
import { $, h } from "../../dom.js";
import { gb } from "../../format.js";
import { fillModelSelect, loadReleases, versionOptions } from "../../models.js";
import { state } from "../../state.js";

const modelSelect = $("#model"), customModel = $("#model-custom");
const versionSelect = $("#claude-version"), cpuSelect = $("#vm-cpus"), memSelect = $("#vm-mem");

function fillResourceSelects(count) {
  const set = state.settings;
  if (!set) return;
  const { cpus: hostCpus, ram_mb: hostRam } = set.limits;
  const cpuChoices = [...new Set([1, 2, 4, 6, 8, 12, 16, hostCpus].filter(n => n <= hostCpus))].sort((a, b) => a - b);
  const memChoices = [1024, 2048, 4096, 6144, 8192, 12288, 16384, 24576, 32768].filter(v => v <= hostRam - 8192);
  cpuSelect.replaceChildren(...cpuChoices.map(n => h("option", { value: String(n), selected: n === set.settings.cpus }, String(n))));
  memSelect.replaceChildren(...memChoices.map(v => h("option", { value: String(v), selected: v === set.settings.memory_mb }, gb(v))));
  updateResourceHint(count);
}

/** How many VMs of the chosen memory still fit, against the `count` about to launch. */
export function updateResourceHint(count) {
  const s = state.status, set = state.settings;
  if (!s || !set) return;
  const mem = Number(memSelect.value || set.settings.memory_mb);
  const free = s.host.ram_mb - 8192 - s.ram_committed_mb;
  const fits = Math.floor(free / mem);
  const hint = $("#res-hint");
  hint.className = `res-hint${fits < count ? " over" : ""}`;
  hint.textContent = fits < count
    ? `Only ${Math.max(fits, 0)} more VM${fits === 1 ? "" : "s"} of ${gb(mem)} fit in free memory; the rest will wait or swap.`
    : `${gb(Math.max(free, 0))} free for VMs: room for ${fits} like this.`;
}

/** Fills every select from the settings, then the versions once they are known. */
export async function fillChoices(count) {
  fillResourceSelects(count);
  fillModelSelect(modelSelect, state.settings?.settings.model ?? "default");
  const [g] = await Promise.all([api("/api/golden"), loadReleases()]);
  versionSelect.replaceChildren(...versionOptions(true, g.ok ? g.data.claude_version : null, ""));
}

const chosenModel = () => (modelSelect.value === "__custom" ? customModel.value.trim() : modelSelect.value) || null;

/** The choices as the launch request wants them; `null` means the default. */
export const chosen = () => ({
  model: chosenModel(),
  claude_version: versionSelect.value || null,
  cpus: Number(cpuSelect.value) || null,
  memory_mb: Number(memSelect.value) || null,
});

/** "Other model ID…" shows a field for it. */
export function initChoices({ onMemoryChange }) {
  memSelect.addEventListener("change", onMemoryChange);
  modelSelect.addEventListener("change", e => {
    const custom = e.target.value === "__custom";
    customModel.hidden = !custom;
    if (custom) customModel.focus();
  });
}
