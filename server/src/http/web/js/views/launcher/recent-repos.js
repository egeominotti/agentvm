/* New VM: the repositories used lately, suggested under the repository field. */
import { h } from "../../dom.js";
import { repoName, shortPath } from "../../format.js";
import { state } from "../../state.js";

const RECENT_KEY = "agentvm.recentRepos";

/** The default repository first, then the ones launched from this browser, then those of the machines. */
export function recentRepos() {
  let saved = [];
  try { saved = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]"); } catch {}
  const preferred = state.settings?.settings.default_repo ? [state.settings.settings.default_repo] : [];
  return [...new Set([...preferred, ...saved, ...state.tasks.map(t => shortPath(t.repo))])].slice(0, 8);
}

export function rememberRepo(p) {
  try { localStorage.setItem(RECENT_KEY, JSON.stringify([p, ...recentRepos().filter(x => x !== p)].slice(0, 8))); } catch {}
}

/** Shows the matching recent repositories in `box` while `input` has the focus. */
export function suggestRepos(input, box, { onInput, onPick }) {
  const show = open => {
    const q = input.value.trim().toLowerCase();
    const list = recentRepos().filter(p => !q || (p.toLowerCase().includes(q) && p.toLowerCase() !== q));
    if (!open || !list.length) { box.hidden = true; return; }
    box.replaceChildren(...list.map(p => h("button", { type: "button", onmousedown: e => { e.preventDefault(); input.value = p; box.hidden = true; onPick(); } },
      h("span", {}, repoName(p)), h("small", {}, p))));
    box.hidden = false;
  };
  input.addEventListener("focus", () => show(true));
  input.addEventListener("input", () => { show(true); onInput(); });
  input.addEventListener("blur", () => setTimeout(() => show(false), 120));
}
