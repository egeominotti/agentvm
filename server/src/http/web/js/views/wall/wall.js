/* The wall: every machine at once, live. */
import { api } from "../../api.js";
import { h } from "../../dom.js";
import { plural } from "../../format.js";
import { byId, loadTasks, state } from "../../state.js";
import { isEnded, isWaiting } from "../../task.js";
import { toast } from "../../ui/toast.js";
import { openLauncher } from "../launcher/launcher.js";
import { Cell } from "./cell.js";

function emptyState() {
  return h("div", { class: "empty" },
    h("img", { class: "mark", src: "/logo.svg", alt: "" }),
    h("h1", {}, "Every terminal is a sealed machine."),
    h("p", {}, "Launch a VM and Claude Code opens inside it with root and every permission, on a fresh clone of your repository. Nothing it does can touch your Mac."),
    h("button", { class: "btn primary", type: "button", onclick: openLauncher }, "Launch your first VM"),
    h("ol", {},
      h("li", {}, h("span", {}, h("b", {}, "Press New VM"), " and pick a repository and, if you like, a first task.")),
      h("li", {}, h("span", {}, h("b", {}, "Work with Claude"), " in the VM terminal, or open a root shell next to it.")),
      h("li", {}, h("span", {}, h("b", {}, "Save to repo"), " turns the work into a branch agent/… in your repository."))));
}

async function clearFinished() {
  const done = state.tasks.filter(isEnded);
  for (const t of done) await api(`/api/tasks/${t.id}`, { method: "DELETE" });
  toast(`Removed ${plural(done.length, "finished machine")}`);
  loadTasks();
}

export class WallView {
  constructor() {
    this.cells = new Map();
    this.grid = h("div", { class: "wall" });
    this.head = h("header", { class: "view-head" });
    this.root = h("div", { style: "flex:1;min-height:0;display:flex;flex-direction:column" }, this.head, this.grid);
  }

  update() {
    const tasks = state.tasks;
    if (!tasks.length) { this.root.replaceChildren(emptyState()); this.cells.clear(); return; }
    if (!this.grid.isConnected) this.root.replaceChildren(this.head, this.grid);
    this.paintHead(tasks);
    for (const [id, cell] of this.cells) if (!byId(id)) { cell.destroy(); this.cells.delete(id); }
    tasks.forEach((t, i) => {
      let cell = this.cells.get(t.id);
      if (!cell) { cell = new Cell(t); this.cells.set(t.id, cell); }
      if (this.grid.children[i] !== cell.root) this.grid.insertBefore(cell.root, this.grid.children[i] ?? null);
      cell.update(t);
    });
  }

  paintHead(tasks) {
    const live = tasks.filter(t => !isEnded(t)).length;
    const waiting = tasks.filter(isWaiting).length;
    const finished = tasks.filter(isEnded);
    if (!this.headBuilt) {
      this.headBuilt = true;
      this.sub = h("span", { class: "sub" });
      this.clearBtn = h("button", { class: "btn ghost small", type: "button", onclick: clearFinished });
      this.head.replaceChildren(h("h1", {}, "Machines"), this.sub, h("span", { class: "spacer" }), this.clearBtn);
    }
    const sub = waiting ? `${plural(live, "running machine")}, ${waiting} waiting for you` : `${plural(live, "running machine")}. Click one to work in it.`;
    if (this.sub.textContent !== sub) this.sub.textContent = sub;
    this.clearBtn.hidden = !finished.length;
    const clearText = `Clear ${finished.length} finished`;
    if (this.clearBtn.textContent !== clearText) this.clearBtn.textContent = clearText;
  }

  /** A hidden tab streams nothing to its previews. */
  visibilityChanged() { for (const c of this.cells.values()) c.stream(); }

  destroy() { for (const c of this.cells.values()) c.destroy(); }
}
