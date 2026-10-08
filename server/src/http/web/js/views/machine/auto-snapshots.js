/* The "Auto" menu next to Snapshot: this machine's automatic snapshot interval. */
import { api } from "../../api.js";
import { h } from "../../dom.js";
import { loadTasks, state } from "../../state.js";
import { toast } from "../../ui/toast.js";

const label = m => (m === 0 ? "off" : m < 60 ? `${m} min` : `${m / 60} h`);

export class AutoSnapshotSelect {
  constructor(id) {
    this.id = id;
    this.el = h("select", { class: "auto-snap", title: "Automatic snapshots of this machine", onchange: e => this.choose(e.target.value) });
  }

  /** Rebuilt only when the interval or the default changes, and never while the menu is open. */
  paint(t) {
    const def = state.settings?.settings.auto_snapshots?.every_min ?? 30;
    const key = `${def}:${t.auto_snapshot_min}`;
    if (key === this.key || document.activeElement === this.el) return;
    this.key = key;
    this.el.replaceChildren(
      h("option", { value: "", selected: t.auto_snapshot_min == null }, `Auto: ${label(def)}`),
      ...[0, 5, 15, 30, 60, 120].map(m => h("option", { value: String(m), selected: t.auto_snapshot_min === m }, m === 0 ? "Auto: off" : `Auto: every ${label(m)}`)));
  }

  /** `v` is minutes, "0" for none, or "" to follow the settings. */
  async choose(v) {
    const r = await api(`/api/tasks/${this.id}/auto-snapshots`, { method: "PUT", body: { every_min: v === "" ? null : Number(v) } });
    toast(r.ok ? (v === "" ? "Snapshots follow the settings" : v === "0" ? "No automatic snapshots for this machine" : `A snapshot every ${v} minutes`) : r.data?.error || "Could not change it", r.ok ? "ok" : "err");
    this.key = null;
    loadTasks();
  }
}
