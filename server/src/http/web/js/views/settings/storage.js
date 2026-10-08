/* Settings › Storage: what agentvm keeps on disk, and cleaning up after closed VMs. */
import { api } from "../../api.js";
import { h } from "../../dom.js";
import { gb, plural } from "../../format.js";
import { fact } from "./fact.js";

export function storageSection({ refresh }) {
  const facts = h("div", { class: "facts" });
  const msg = h("span", { class: "msg" });

  async function cleanup() {
    const r = await api("/api/storage/cleanup", { method: "POST" });
    msg.className = `msg ${r.ok ? "ok" : "err"}`;
    msg.textContent = r.ok ? `Deleted ${plural(r.data.removed, "job folder")}.` : "Cleanup failed.";
    refresh();
  }

  return {
    body: [h("p", { class: "lede" }, "Disks of closed VMs are deleted at once. Their logs stay until you clean them up."),
      facts, h("div", { class: "row-actions" }, h("button", { class: "btn", type: "button", onclick: () => cleanup() }, "Delete logs of closed VMs"), msg)],
    /** `d` is the usage as GET /api/storage reports it. */
    paint(d) {
      facts.replaceChildren(fact("Job logs", gb(d.jobs_mb)), fact("Running VM disks", gb(d.vm_disks_mb)), fact("Snapshots", gb(d.snapshots_mb)), fact("VM image", gb(d.golden_mb)));
    },
  };
}
