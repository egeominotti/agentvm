/* How a machine ended, and what to do with its branch. */
import { h } from "../../dom.js";
import { plural, shortPath } from "../../format.js";
import { kind } from "../../task.js";

/** [title, explanation] for a failure reason reported by the server. */
function explain(reason = "") {
  if (reason === "timeout") return ["Time limit reached", "The agent was stopped after the time limit in Settings."];
  if (reason === "guest_no_result") return ["The VM shut down without a result", "Check console.log in the job folder."];
  if (reason.startsWith("claude_exit")) return ["Claude stopped with an error", ""];
  if (reason.startsWith("vm_error")) return ["The VM did not start", "Virtualization.framework reported an error."];
  if (reason.startsWith("fetch_failed")) return ["Could not import the branch", "The work finished but git fetch failed."];
  return ["Failed", ""];
}

/** A command with a button that copies it. */
function copyRow(text) {
  const btn = h("button", { class: "btn small", type: "button" }, "Copy");
  btn.addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(text); btn.textContent = "Copied"; } catch { btn.textContent = "Select it"; }
    setTimeout(() => (btn.textContent = "Copy"), 1500);
  });
  return h("div", { class: "cmd" }, h("code", {}, text), btn);
}

export function outcome(t) {
  const s = t.status.state;
  const repo = shortPath(t.repo).replace(/ /g, "\\ ");
  const [cls] = kind(t);
  const box = h("div", { class: `outcome ${cls}` });
  if (s === "done") box.append(h("div", { class: "title" }, `${t.branch}: ${plural(t.status.commits, "commit")}`),
    h("div", { class: "why" }, "Already in your repository. To try it or merge it:"), copyRow(`git -C ${repo} switch ${t.branch}`), copyRow(`git -C ${repo} merge ${t.branch}`));
  else if (s === "no_changes") box.append(h("div", { class: "title" }, "Closed without file changes"), h("div", { class: "why" }, "No branch was created."));
  else if (s === "stopped") box.append(h("div", { class: "title" }, "Force stopped"), h("div", { class: "why" }, "Anything saved before is still on the branch."));
  else { const [title, why] = explain(t.status.reason); box.append(h("div", { class: "title" }, title), why && h("div", { class: "why" }, why), h("div", { class: "code" }, t.status.reason ?? "")); }
  return box;
}
