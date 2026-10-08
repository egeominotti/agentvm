/* Snapshots backed up to S3, brought back to this Mac on demand. */
import { api } from "../../api.js";
import { h } from "../../dom.js";
import { gb, repoName } from "../../format.js";
import { confirmButton } from "../../ui/confirm-button.js";

function backupRow(b, { reload, restored }) {
  const msg = h("span", { class: "msg" });
  const restore = h("button", { class: "btn", type: "button", disabled: b.local, title: b.local ? "Already on this Mac" : "", onclick: async () => {
    restore.disabled = true; msg.className = "msg"; msg.textContent = "Downloading…";
    const x = await api(`/api/backups/${b.snapshot.id}/restore`, { method: "POST" });
    msg.className = `msg ${x.ok ? "ok" : "err"}`; msg.textContent = x.ok ? "Now in your snapshots" : x.data?.error || "Download failed";
    if (x.ok) restored();
  } }, b.local ? "On this Mac" : "Bring to this Mac");
  const del = confirmButton("Delete from S3", async () => {
    const x = await api(`/api/backups/${b.snapshot.id}`, { method: "DELETE" });
    if (x.ok) reload(); else { msg.className = "msg err"; msg.textContent = x.data?.error || "Delete failed"; }
  });
  return h("article", { class: "snap remote-snap" },
    h("div", { class: "snap-main" }, h("b", { class: "snap-name" }, b.snapshot.name),
      h("div", { class: "snap-meta" }, h("span", {}, repoName(b.snapshot.repo)), h("span", {}, new Date(b.snapshot.created_at * 1000).toLocaleString()), h("span", {}, `${gb(b.archive_mb)} compressed`))),
    msg, del, restore);
}

/** The "In S3" section's children. `reload` lists the bucket again; `restored` follows a download. */
export async function backupsList(actions) {
  const r = await api("/api/backups");
  if (!r.ok) {
    const notSet = /not configured/i.test(r.data?.error || "");
    return [h("h2", {}, "In S3"), h("p", { class: "hint" }, notSet ? "Set up a bucket in Settings → Backups to S3 to keep copies off this Mac." : r.data?.error || "Could not reach S3.")];
  }
  const rows = r.data.map(b => backupRow(b, actions));
  return [h("h2", {}, "In S3"), rows.length ? h("div", { class: "snap-list" }, rows) : h("p", { class: "hint" }, "No backups in the bucket yet.")];
}
