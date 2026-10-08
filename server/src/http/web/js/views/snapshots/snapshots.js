/* Saved copies of whole VMs, restorable into new machines. */
import { api } from "../../api.js";
import { h } from "../../dom.js";
import { gb, repoName } from "../../format.js";
import { modelLabel } from "../../models.js";
import { loadTasks } from "../../state.js";
import { confirmButton } from "../../ui/confirm-button.js";
import { backupsList } from "./backups.js";

export class SnapshotsView {
  constructor() {
    this.root = h("div", { class: "snapshots" });
    this.built = false;
  }

  async update() {
    if (this.built) return;
    this.built = true;
    await this.refresh();
  }

  async refresh() {
    const r = await api("/api/snapshots");
    const list = r.ok ? r.data : [];
    const fileInput = h("input", { type: "file", accept: ".zst,.tar,.gz", hidden: true, onchange: e => this.importFile(e.target.files[0]) });
    this.importMsg = h("span", { class: "msg" });
    const head = h("header", { class: "view-head" }, h("h1", {}, "Snapshots"),
      h("span", { class: "sub" }, "Instant copies of whole VMs: files, installed packages, Claude's conversation."),
      h("span", { class: "spacer" }), this.importMsg,
      h("button", { class: "btn small", type: "button", onclick: () => fileInput.click() }, "Import from file"), fileInput);
    this.remote = h("section", { class: "remote" });
    this.loadRemote();
    if (!list.length) {
      this.root.replaceChildren(h("div", { class: "snap-inner" }, head, h("div", { class: "card empty-card" },
        h("b", {}, "No snapshots yet"), h("p", { class: "hint" }, "Open a running machine and press Snapshot. Restoring starts a new VM exactly from that point, with Claude continuing its conversation.")), this.remote));
      return;
    }
    this.root.replaceChildren(h("div", { class: "snap-inner" }, head, h("div", { class: "snap-list" }, list.map(sn => this.row(sn))), this.remote));
  }

  row(sn) {
    const msg = h("span", { class: "msg" });
    const restore = h("button", { class: "btn", type: "button", onclick: async () => {
      restore.disabled = true;
      const r = await api(`/api/snapshots/${sn.id}/restore`, { method: "POST" });
      if (r.ok) { await loadTasks(); location.hash = `#/vm/${r.data.id}`; }
      else { msg.className = "msg err"; msg.textContent = r.data?.error || "Restore failed"; restore.disabled = false; }
    } }, "Restore");
    const del = confirmButton("Delete", async () => {
      const r = await api(`/api/snapshots/${sn.id}`, { method: "DELETE" });
      if (r.ok) this.refresh(); else { msg.className = "msg err"; msg.textContent = r.data?.error || "Delete failed"; }
    });
    const download = h("a", { class: "btn ghost", href: `/api/snapshots/${sn.id}/export`, download: `${sn.id}.tar.zst`, title: "Download a .tar.zst you can import elsewhere" }, "Download");
    const backup = h("button", { class: "btn ghost", type: "button", onclick: async () => {
      backup.disabled = true; msg.className = "msg"; msg.textContent = "Uploading to S3…";
      const r = await api(`/api/snapshots/${sn.id}/backup`, { method: "POST" });
      msg.className = `msg ${r.ok ? "ok" : "err"}`;
      msg.textContent = r.ok ? `Backed up (${gb(r.data.archive_mb)} compressed)` : r.data?.error || "Backup failed";
      backup.disabled = false;
      if (r.ok) this.loadRemote();
    } }, "Back up to S3");
    return h("article", { class: "snap" },
      h("div", { class: "snap-main" },
        h("b", { class: "snap-name" }, sn.name),
        h("div", { class: "snap-meta" },
          h("span", {}, repoName(sn.repo)),
          h("span", {}, new Date(sn.created_at * 1000).toLocaleString()),
          h("span", {}, `disk image ${gb(sn.size_mb)}`),
          sn.cpus ? h("span", {}, `${sn.cpus} vCPUs, ${gb(sn.memory_mb)}`) : null,
          h("span", {}, modelLabel(sn.model)),
          sn.auto ? h("span", { class: "tag" }, "Automatic") : null)),
      msg, download, backup, del, restore);
  }

  async importFile(file) {
    if (!file) return;
    this.importMsg.className = "msg";
    this.importMsg.textContent = `Importing ${file.name}…`;
    try {
      const res = await fetch("/api/snapshots/import", { method: "POST", body: file, headers: { "content-type": "application/octet-stream" } });
      const data = await res.json().catch(() => ({}));
      this.importMsg.className = `msg ${res.ok ? "ok" : "err"}`;
      this.importMsg.textContent = res.ok ? `Imported ${data.name}` : data.error || "Import failed";
      if (res.ok) this.refresh();
    } catch { this.importMsg.className = "msg err"; this.importMsg.textContent = "Import failed"; }
  }

  /** Fills the S3 section; it is replaced on every refresh, so this looks it up once the bucket answered. */
  async loadRemote() {
    const kids = await backupsList({
      reload: () => this.loadRemote(),
      restored: () => { this.built = false; this.update(); },
    });
    this.remote.replaceChildren(...kids);
  }

  destroy() {}
}
