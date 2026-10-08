/* Files dropped on a terminal are copied into the VM and their paths typed, as the Mac's Terminal does. */
import { h } from "../dom.js";
import { toast } from "../ui/toast.js";

/** The hint shown over the terminal while files are dragged over it. */
export const dropHint = () =>
  h("div", { class: "drop-hint" }, h("b", {}, "Drop to copy into the VM"), h("span", {}, "The files go to /mnt/job/uploads and their paths are typed in the terminal."));

/** Makes `screen` a drop target for files while `enabled()`; `onFiles` gets the dropped files. */
export function acceptFileDrops(screen, { enabled, onFiles }) {
  screen.addEventListener("dragover", e => {
    if (![...e.dataTransfer.types].includes("Files") || !enabled()) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    screen.classList.add("dropping");
  });
  screen.addEventListener("dragleave", e => { if (!screen.contains(e.relatedTarget)) screen.classList.remove("dropping"); });
  screen.addEventListener("drop", e => { e.preventDefault(); screen.classList.remove("dropping"); onFiles([...e.dataTransfer.files]); });
}

/** Copies the files into the VM one by one; resolves to the paths of those that made it. */
export async function uploadFiles(taskId, files) {
  const paths = [];
  for (const f of files) {
    toast(`Copying ${f.name} into the VM…`);
    try {
      const res = await fetch(`/api/tasks/${taskId}/upload`, { method: "POST", body: f, headers: { "x-file-name": encodeURIComponent(f.name) } });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) { toast(data.error || `Could not copy ${f.name}`, "err"); continue; }
      paths.push(data.path);
    } catch { toast(`Could not copy ${f.name}`, "err"); }
  }
  return paths;
}

const shellQuote = p => (/^[\w@%+=:,./-]+$/.test(p) ? p : `'${p.replace(/'/g, "'\\''")}'`);

/** Types the paths, shell-quoted, at the terminal's prompt. */
export function typePaths(xterm, paths) {
  xterm.paste(paths.map(shellQuote).join(" ") + " ");
  xterm.focus();
  toast(paths.length === 1 ? `In the VM: ${paths[0]}` : `${paths.length} files in /mnt/job/uploads`);
}
