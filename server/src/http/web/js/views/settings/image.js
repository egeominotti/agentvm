/* Settings › VM image: the Debian image every VM starts from, and its rebuild. */
import { api } from "../../api.js";
import { h } from "../../dom.js";
import { gb } from "../../format.js";
import { loadReleases, versionOptions } from "../../models.js";
import { fact } from "./fact.js";

export function imageSection({ draft, set, refresh }) {
  const facts = h("div", { class: "facts" });
  const log = h("pre", { class: "log", hidden: true });
  const rebuildBtn = h("button", { class: "btn", type: "button", onclick: () => rebuild() }, "Rebuild image");
  const msg = h("span", { class: "msg" });
  const versionSelect = h("select", { onchange: e => set("claude_version", e.target.value) }, versionOptions(false, null, draft.claude_version));
  loadReleases().then(() => versionSelect.replaceChildren(...versionOptions(false, null, draft.claude_version)));

  async function rebuild() {
    const r = await api("/api/golden/rebuild", { method: "POST" });
    msg.className = `msg ${r.ok ? "" : "err"}`;
    msg.textContent = r.ok ? "Rebuilding, about 2 minutes…" : r.data?.error || "Could not start the rebuild.";
    refresh();
  }

  /** `d` is the image as GET /api/golden reports it. */
  function paint(d) {
    facts.replaceChildren(
      fact("Status", d.rebuilding ? "Rebuilding" : d.exists ? "Ready" : "Missing"),
      fact("Claude Code", d.claude_version ?? "unknown"),
      fact("Built", d.built_at ? new Date(d.built_at * 1000).toLocaleString() : "never"),
      fact("Size on disk", gb(d.size_mb)));
    rebuildBtn.disabled = d.rebuilding;
    log.hidden = !(d.rebuilding || d.last_result);
    log.textContent = d.log_tail;
    if (d.last_result && !d.rebuilding) { msg.className = `msg ${d.last_result === "ok" ? "ok" : "err"}`; msg.textContent = d.last_result === "ok" ? "Image rebuilt." : `Rebuild ${d.last_result}.`; }
  }

  return {
    body: [h("p", { class: "lede" }, "Every VM starts as an instant copy of this Debian 13 image with Claude Code preinstalled. Rebuild it to update Claude Code and the system packages; running VMs are not affected."),
      facts,
      h("div", { class: "set narrow" }, h("label", {}, "Claude Code version for the image"), versionSelect,
        h("p", { class: "hint" }, "Used by the next rebuild. A single launch can still pick another version; auto-update is off inside the VMs.")),
      h("div", { class: "row-actions" }, rebuildBtn, msg), log],
    changed(key) {
      if (key === "claude_version") msg.textContent = "Rebuild the image to apply the new version.";
    },
    paint,
  };
}
