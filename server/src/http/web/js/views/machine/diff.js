/* The changes a finished machine brought to its branch. */
import { h } from "../../dom.js";
import { plural } from "../../format.js";

/** Splits a unified diff into files with their changed lines and counts. */
function parseDiff(text) {
  const files = [];
  let cur = null;
  for (const line of text.split("\n")) {
    if (line.startsWith("diff --git")) { const m = line.match(/ b\/(.+)$/); cur = { path: m ? m[1] : line, add: 0, del: 0, lines: [] }; files.push(cur); }
    else if (!cur || /^(index |--- |\+\+\+ |new file mode|deleted file mode|similarity |rename )/.test(line)) continue;
    else { if (line.startsWith("+")) cur.add++; else if (line.startsWith("-")) cur.del++; cur.lines.push(line); }
  }
  return files;
}

/** Appends the task's diff to `container`: a summary, then one collapsible block per file. */
export async function appendDiff(t, container) {
  const r = await fetch(`/api/tasks/${t.id}/diff`);
  if (!r.ok) return;
  const files = parseDiff(await r.text());
  if (!files.length) return;
  const add = files.reduce((n, f) => n + f.add, 0), del = files.reduce((n, f) => n + f.del, 0);
  container.append(h("div", { class: "diff-summary" }, `${plural(files.length, "file")} changed, `, h("span", { class: "plus" }, `+${add}`), " ", h("span", { class: "minus" }, `−${del}`)),
    ...files.map((f, i) => h("details", { class: "diff-file", open: files.length <= 4 || i === 0 },
      h("summary", {}, h("span", { class: "path", title: f.path }, f.path), h("span", { class: "plus" }, `+${f.add}`), h("span", { class: "minus" }, `−${f.del}`)),
      h("div", { class: "diff-body" }, f.lines.map(l => h("div", { class: l.startsWith("@@") ? "h" : l.startsWith("+") ? "a" : l.startsWith("-") ? "d" : "" }, l))))));
}
