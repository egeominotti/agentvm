/* Settings: full width, section index on the left, every change saved on the spot.
 * Each section is built by its own module from a shared context and returns
 * `{ body, changed?(key, value) }` plus whatever the view asks of it below. */
import { api } from "../../api.js";
import { $, h } from "../../dom.js";
import { loadSettings, loadStatus, state } from "../../state.js";
import { accountSection } from "./account.js";
import { agentSection } from "./agent.js";
import { autoSnapshotsSection } from "./auto-snapshots.js";
import { imageSection } from "./image.js";
import { notificationsSection } from "./notifications.js";
import { resourcesSection } from "./resources.js";
import { s3Section } from "./s3.js";
import { storageSection } from "./storage.js";

/** [id, title, factory], in page order. */
const SECTIONS = [
  ["resources", "Resources", resourcesSection],
  ["agent", "Agent", agentSection],
  ["account", "Claude account", accountSection],
  ["notifications", "Notifications", notificationsSection],
  ["snapshots", "Automatic snapshots", autoSnapshotsSection],
  ["image", "VM image", imageSection],
  ["storage", "Storage", storageSection],
  ["backups", "Backups to S3", s3Section],
];

export class SettingsView {
  constructor() {
    this.root = h("div", { class: "settings" });
    this.built = false;
    this.saveTimer = null;
  }

  /** Built once; live parts refresh on actions and while the image is rebuilding. */
  async update() {
    if (this.built) return;
    this.built = true;
    await loadSettings();
    this.draft = { ...state.settings.settings };
    const ctx = { draft: this.draft, set: (key, value) => this.set(key, value), refresh: () => this.refreshLive() };
    this.sections = Object.fromEntries(SECTIONS.map(([id, , make]) => [id, make(ctx)]));
    this.navLinks = SECTIONS.map(([id, label]) => h("a", { href: `#/settings`, "data-target": id, onclick: e => { e.preventDefault(); this.scrollTo(id); } }, label));
    this.content = h("div", { class: "settings-content" },
      h("header", { class: "settings-head" }, h("h1", {}, "Settings"), this.saveState = h("span", { class: "save-state" }, "Changes are saved automatically")),
      h("div", { class: "settings-grid" }, SECTIONS.map(([id, label]) => h("section", { class: `card span-${id}`, id: `set-${id}`, "data-section": id }, h("h2", {}, label), this.sections[id].body))));
    this.root.replaceChildren(h("nav", { class: "settings-nav", "aria-label": "Settings sections" }, this.navLinks), this.content);
    this.content.addEventListener("scroll", () => this.spy());
    this.spy();
    this.refreshLive();
  }

  scrollTo(id) {
    $(`#set-${id}`, this.root)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  /** Highlights the section being read in the index. */
  spy() {
    const top = this.content.getBoundingClientRect().top + 80;
    let active = "resources";
    for (const sec of this.content.querySelectorAll("[data-section]")) if (sec.getBoundingClientRect().top <= top) active = sec.dataset.section;
    for (const a of this.navLinks) a.setAttribute("aria-current", String(a.dataset.target === active));
  }

  // ----- auto-save -----
  set(key, value) {
    this.draft[key] = value;
    for (const section of Object.values(this.sections)) section.changed?.(key, value);
    this.saveState.className = "save-state pending";
    this.saveState.textContent = "Saving…";
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => this.save(), 450);
  }

  async save() {
    const r = await api("/api/settings", { method: "PUT", body: this.draft });
    const err = r.ok ? "" : r.data?.error || "Could not save.";
    const resourceError = /VMs|vCPU|memory/i.test(err) && !/model|Claude Code version/i.test(err);
    this.sections.resources.showError(resourceError ? err : "");
    this.sections.agent.showError(err && !resourceError ? err : "");
    this.saveState.className = `save-state ${r.ok ? "ok" : "err"}`;
    this.saveState.textContent = r.ok ? "All changes saved" : "Not saved: fix the highlighted value";
    if (r.ok) { state.settings = r.data; loadStatus(); }
  }

  /** The token, the image and the storage, as they are now; polls while the image rebuilds. */
  async refreshLive() {
    this.sections.account.paint();
    const [g, st] = await Promise.all([api("/api/golden"), api("/api/storage")]);
    if (g.ok) {
      this.sections.image.paint(g.data);
      if (g.data.rebuilding) setTimeout(() => !this.destroyed && this.refreshLive(), 1500);
    }
    if (st.ok) this.sections.storage.paint(st.data);
  }

  destroy() {
    this.destroyed = true;
    clearTimeout(this.saveTimer);
  }
}
