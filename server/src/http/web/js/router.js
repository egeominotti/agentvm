/* Hash routing: one view mounted in <main> at a time.
 * A view is `{ root, update(), destroy() }`, optionally with `visibilityChanged()`. */
import { $ } from "./dom.js";
import { renderSide } from "./sidebar.js";
import { MachineView } from "./views/machine/machine.js";
import { SettingsView } from "./views/settings/settings.js";
import { SnapshotsView } from "./views/snapshots/snapshots.js";
import { WallView } from "./views/wall/wall.js";

/** The view a hash shows: `key` tells whether the mounted one already is it. */
function resolve(hash) {
  const vm = hash.match(/^#\/vm\/(.+)$/);
  if (vm) return { key: `vm/${vm[1]}`, make: () => new MachineView(vm[1]) };
  if (hash.startsWith("#/snapshots")) return { key: "snapshots", make: () => new SnapshotsView() };
  if (hash.startsWith("#/settings")) return { key: "settings", make: () => new SettingsView() };
  return { key: "wall", make: () => new WallView() };
}

let current = null, currentKey = null;

function mount(key, view) {
  current?.destroy();
  current = view;
  currentKey = key;
  $("#view").replaceChildren(view.root);
  view.update();
}

export function route() {
  const hash = location.hash || "#/wall";
  for (const a of document.querySelectorAll("[data-nav]")) a.setAttribute("aria-current", hash.startsWith(`#/${a.dataset.nav}`) || (a.dataset.nav === "wall" && hash.startsWith("#/vm/")) ? "page" : "false");
  renderSide();
  const { key, make } = resolve(hash);
  if (key !== currentKey) mount(key, make());
}

/** Repaints the mounted view with the latest state. */
export function updateView() { current?.update(); }

export function startRouter() {
  window.addEventListener("hashchange", route);
  document.addEventListener("visibilitychange", () => current?.visibilityChanged?.());
}
