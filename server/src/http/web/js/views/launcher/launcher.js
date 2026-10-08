/* New VM: the dialog that launches machines (⌘K). */
import { api } from "../../api.js";
import { $ } from "../../dom.js";
import { loadTasks, state } from "../../state.js";
import { chosen, fillChoices, initChoices, updateResourceHint } from "./choices.js";
import { recentRepos, rememberRepo, suggestRepos } from "./recent-repos.js";

const dialog = $("#launch-dialog");
const form = $("#launcher");
const repoEl = $("#repo"), promptEl = $("#prompt");

/** One task per VM: the whole text, or each line of it with "One VM per line". */
function prompts() {
  const text = promptEl.value.trim();
  if (!$("#batch").checked) return [text];
  const lines = text.split("\n").map(l => l.replace(/^\s*[-*•\d.)]+\s+/, "").trim()).filter(Boolean);
  return lines.length ? lines : [""];
}
const updateHint = () => updateResourceHint(prompts().length);

/** The launch button: how many VMs, and whether they can start at all. */
export function updateLaunch() {
  const n = prompts().length;
  const btn = $("#launch");
  btn.firstChild.textContent = n > 1 ? `Launch ${n} VMs ` : "Launch VM ";
  btn.disabled = !state.status?.golden || !state.status?.token || !repoEl.value.trim();
}

function autosize() {
  promptEl.style.height = "76px";
  const want = promptEl.scrollHeight + 2;
  promptEl.style.height = Math.min(Math.max(want, 76), 220) + "px";
  promptEl.classList.toggle("tall", want > 220);
}

export function openLauncher() {
  if (dialog.open) return;
  fillChoices(prompts().length);
  dialog.showModal();
  (repoEl.value ? promptEl : repoEl).focus();
}

/** Prefills the repository with the most recent one. */
export function prefillLauncher() {
  repoEl.value = recentRepos()[0] ?? "";
  updateLaunch();
}

async function launch(e) {
  e.preventDefault();
  const repo_path = repoEl.value.trim();
  const { model, claude_version, cpus, memory_mb } = chosen();
  const err = $("#form-error");
  err.hidden = true;
  $("#launch").disabled = true;
  const ids = [];
  for (const prompt of prompts()) {
    const r = await api("/api/tasks", { method: "POST", body: { repo_path, prompt, interactive: true, model, claude_version, cpus, memory_mb } });
    if (!r.ok) { err.textContent = r.data?.error || "Could not launch the VM."; err.hidden = false; break; }
    ids.push(r.data.id);
  }
  if (ids.length) {
    rememberRepo(repo_path);
    promptEl.value = "";
    autosize();
    dialog.close();
    await loadTasks();
    location.hash = ids.length === 1 ? `#/vm/${ids[0]}` : "#/wall";
  }
  updateLaunch();
}

export function initLauncher() {
  initChoices({ onMemoryChange: updateHint });
  suggestRepos(repoEl, $("#suggest"), { onInput: updateLaunch, onPick: () => { updateLaunch(); promptEl.focus(); } });
  $("#new-vm").addEventListener("click", openLauncher);
  document.addEventListener("keydown", e => {
    if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); openLauncher(); }
  });
  dialog.addEventListener("click", e => { if (e.target === dialog) dialog.close(); });
  $("#launch-close").addEventListener("click", () => dialog.close());
  promptEl.addEventListener("input", () => { autosize(); updateLaunch(); updateHint(); });
  $("#batch").addEventListener("change", () => { updateLaunch(); updateHint(); });
  form.addEventListener("keydown", e => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); form.requestSubmit(); }
  });
  form.addEventListener("submit", launch);
}
