/* Claude models and Claude Code versions to choose from. */
import { api } from "./api.js";
import { h } from "./dom.js";

/** [id, name, description]. */
export const MODELS = [
  ["default", "Account default", "Whatever Claude Code picks"],
  ["opus", "Opus", "Most capable"],
  ["sonnet", "Sonnet", "Fast, great for most work"],
  ["haiku", "Haiku", "Fastest and lightest"],
  ["fable", "Fable", "Fable family"],
  ["opusplan", "Opus plans, Sonnet builds", "Opus in plan mode, Sonnet otherwise"],
  ["opus[1m]", "Opus, 1M context", "Long context window"],
  ["sonnet[1m]", "Sonnet, 1M context", "Long context window"],
];
export const isKnownModel = id => MODELS.some(m => m[0] === id);
export const modelLabel = id => (MODELS.find(m => m[0] === id)?.[1]) ?? id;

/** Every model, plus "Other model ID…" selected when `current` is not one of them. */
export function fillModelSelect(select, current) {
  select.replaceChildren(
    ...MODELS.map(([v, l]) => h("option", { value: v, selected: v === current }, l)),
    h("option", { value: "__custom", selected: !isKnownModel(current) && !!current }, "Other model ID…"));
}

let releases = null;
/** Published Claude Code versions, fetched once. */
export async function loadReleases() {
  if (releases) return releases;
  const r = await api("/api/claude/versions");
  if (r.ok) releases = r.data;
  return releases;
}

/** Options for a Claude Code version select; exact versions once `loadReleases()` has them. */
export function versionOptions(includeImage, imageVersion, current) {
  const opts = [];
  if (includeImage) opts.push(h("option", { value: "" }, imageVersion ? `Image version (${imageVersion})` : "Image version"));
  if (releases) {
    opts.push(h("option", { value: "latest", selected: current === "latest" }, `Latest (${releases.latest})`));
    opts.push(h("option", { value: "stable", selected: current === "stable" }, `Stable (${releases.stable})`));
    opts.push(h("optgroup", { label: "Exact version" }, releases.versions.map(v => h("option", { value: v, selected: current === v }, v))));
  } else {
    for (const v of ["latest", "stable"]) opts.push(h("option", { value: v, selected: current === v }, v[0].toUpperCase() + v.slice(1)));
    if (current && !["latest", "stable", ""].includes(current)) opts.push(h("option", { value: current, selected: true }, current));
  }
  return opts;
}
