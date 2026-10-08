/* One machine, full size, with its telemetry. */
import { api } from "../../api.js";
import { h } from "../../dom.js";
import { plural } from "../../format.js";
import { byId, loadTasks } from "../../state.js";
import { isEnded, kind, titleOf } from "../../task.js";
import { acceptFileDrops, dropHint, typePaths, uploadFiles } from "../../terminal/file-drop.js";
import { VmTerminal } from "../../terminal/vm-terminal.js";
import { bootPanel } from "../../ui/boot-panel.js";
import { toast } from "../../ui/toast.js";
import { AutoSnapshotSelect } from "./auto-snapshots.js";
import { ConversationPanel } from "./conversation/panel.js";
import { DiagnosticsPanel } from "./diagnostics.js";
import { appendDiff } from "./diff.js";
import { outcome } from "./outcome.js";
import { PortsBar } from "./ports.js";
import { TelemetryPanel } from "./telemetry/panel.js";

export class MachineView {
  constructor(id) {
    this.id = id;
    this.session = "claude";
    this.terms = {};
    this.showPanel = localStorage.getItem("agentvm.panel") !== "off";
    this.titleEl = h("span", { class: "title" });

    this.segBtns = ["claude", "shell"].map(s => h("button", { type: "button", "aria-pressed": String(s === "claude"), onclick: () => this.show(s) }, s === "claude" ? "Claude" : "Root shell"));
    this.saveBtn = h("button", { class: "btn", type: "button", onclick: () => this.save() }, "Save to repo");
    this.snapBtn = h("button", { class: "btn", type: "button", title: "Save a copy of this whole VM that you can restore later", onclick: () => this.snapshot() }, "Snapshot");
    this.closeBtn = h("button", { class: "btn", type: "button", onclick: () => this.close() }, "Close VM");
    this.auto = new AutoSnapshotSelect(id);
    this.stopBtn = h("button", { class: "btn ghost danger", type: "button", title: "Power off now without saving", onclick: () => this.stop() }, "Force stop");
    this.panelBtn = h("button", { class: "btn ghost", type: "button", onclick: () => this.togglePanel() }, "Telemetry");
    this.diag = new DiagnosticsPanel(id);
    this.conv = new ConversationPanel(id);
    this.convBtn = h("button", { class: "btn ghost", type: "button", title: "Claude's conversation and usage, kept after the VM is closed", onclick: () => this.conv.toggle(!isEnded(byId(this.id) ?? {})) }, "History");
    this.diagBtn = h("button", { class: "btn ghost", type: "button", title: "Why it failed, its timeline and logs", onclick: () => this.diag.toggle() }, "Diagnostics");
    this.seg = h("div", { class: "seg" }, this.segBtns);
    this.toolbar = h("header", { class: "toolbar" }, h("a", { class: "crumb", href: "#/wall" }, "Machines"), h("span", { class: "crumb-sep" }, "›"),
      h("span", { class: "dot" }), this.titleEl, this.seg, h("div", { class: "snap-group" }, this.snapBtn, this.auto.el), this.saveBtn, this.closeBtn, this.stopBtn, this.convBtn, this.diagBtn, this.panelBtn);
    this.overlay = h("div", { class: "overlay" });
    this.ports = new PortsBar();
    this.screen = h("div", { class: "screen" }, this.overlay, dropHint());
    acceptFileDrops(this.screen, { enabled: () => byId(this.id)?.status.state === "running", onFiles: files => this.drop(files) });
    this.result = h("div", { class: "result" });
    this.result.hidden = true;
    this.stage = h("section", { class: "stage" }, this.toolbar, this.ports.el, this.screen, this.result, this.diag.el, this.conv.el);
    this.tele = new TelemetryPanel(id);
    this.panel = h("aside", { class: "panel", "aria-label": "Telemetry" }, this.tele.el);
    this.root = h("div", { class: "focus" }, this.stage, this.panel);
  }

  update() {
    const t = byId(this.id);
    if (!t) { this.overlayText("Machine not found", "It may belong to a previous server session."); return; }
    const [cls, label] = kind(t);
    this.root.className = `focus ${cls}${this.showPanel ? "" : " no-panel"}`;
    this.panel.hidden = !this.showPanel;
    this.toolbar.className = `toolbar ${cls}`;
    this.titleEl.textContent = titleOf(t);
    this.titleEl.title = `${t.prompt || "(no first task)"}\n${t.repo}`;
    const s = t.status.state;
    const ended = isEnded(t);
    for (const b of [this.saveBtn, this.closeBtn, this.snapBtn, this.seg, this.auto.el]) b.hidden = !t.interactive || ended;
    this.auto.paint(t);
    this.saveBtn.disabled = this.closeBtn.disabled = this.snapBtn.disabled = s !== "running";
    this.stopBtn.hidden = ended;
    if (this.showPanel) this.tele.update(t, label);
    this.ports.update(ended ? [] : t.ports || []);
    if (ended) return this.ended(t);
    if (!t.interactive) return this.overlayText("Running without a terminal", "Started from the API in automatic mode.");
    if (s !== "running" || !t.ready) {
      this.screen.hidden = false;
      this.result.hidden = true;
      this.overlay.hidden = false;
      if (!this.boot || !this.overlay.contains(this.boot)) { this.boot = bootPanel(); this.overlay.replaceChildren(this.boot); }
      this.boot.update(t);
      if (s === "running") this.ensureTerm(this.session).connect(); // connect behind the curtain
      return;
    }
    if (!this.overlay.hidden) { this.overlay.classList.add("lift"); setTimeout(() => { this.overlay.hidden = true; this.overlay.classList.remove("lift"); }, 260); }
    this.ensureTerm(this.session).connect();
  }

  overlayText(title, text) {
    this.screen.hidden = false;
    this.result.hidden = true;
    this.overlay.hidden = false;
    this.overlay.replaceChildren(h("div", {}, h("img", { class: "mark", src: "/logo.svg", alt: "" }), h("b", {}, title), text));
  }

  /** The terminal of `session`, created on first use; only the current session's one is shown. */
  ensureTerm(session) {
    let term = this.terms[session];
    if (!term) {
      term = this.terms[session] = new VmTerminal(this.id, session, { fontSize: 13 });
      this.screen.append(term.el);
    }
    for (const [name, other] of Object.entries(this.terms)) other.el.hidden = name !== this.session;
    return term;
  }

  show(session) {
    this.session = session;
    for (const b of this.segBtns) b.setAttribute("aria-pressed", String(b.textContent.startsWith(session === "claude" ? "Claude" : "Root")));
    if (byId(this.id)?.status.state === "running") {
      const term = this.ensureTerm(session);
      term.connect();
      term.fitSoon();
      term.xterm.focus();
    }
  }

  togglePanel() {
    this.showPanel = !this.showPanel;
    try { localStorage.setItem("agentvm.panel", this.showPanel ? "on" : "off"); } catch {}
    this.update();
    this.terms[this.session]?.fitSoon();
  }

  /** Copies dropped files into the VM, then types their paths, as the Mac's Terminal does. */
  async drop(files) {
    const paths = await uploadFiles(this.id, files);
    if (paths.length) typePaths(this.ensureTerm(this.session).xterm, paths);
  }

  async save() {
    this.saveBtn.disabled = true;
    const r = await api(`/api/tasks/${this.id}/save`, { method: "POST" });
    const elsewhere = r.ok && r.data.branch !== `agent/${this.id}`;
    toast(r.ok ? (r.data.commits ? `Saved ${plural(r.data.commits, "commit")} to ${r.data.branch}${elsewhere ? ` (agent/${this.id} has your own commits or is checked out)` : ""}` : "Nothing to save yet") : r.data?.error || "Save failed", r.ok ? "ok" : "err");
    this.saveBtn.disabled = false;
    this.terms[this.session]?.xterm.focus();
  }

  async snapshot() {
    this.snapBtn.disabled = true;
    const r = await api(`/api/tasks/${this.id}/snapshot`, { method: "POST", body: {} });
    toast(r.ok ? `Snapshot saved: ${r.data.name}` : r.data?.error || "Snapshot failed", r.ok ? "ok" : "err");
    this.snapBtn.disabled = false;
    this.terms[this.session]?.xterm.focus();
  }

  async close() {
    this.closeBtn.disabled = this.saveBtn.disabled = true;
    this.closeBtn.textContent = "Closing…";
    const r = await api(`/api/tasks/${this.id}/close`, { method: "POST" });
    if (!r.ok) {
      // The VM stays up and nothing is lost: say why and give the buttons back.
      toast(r.data?.error || "Could not close the VM", "err");
      this.closeBtn.textContent = "Close VM";
      this.closeBtn.disabled = this.saveBtn.disabled = false;
    }
    loadTasks();
  }

  async stop() {
    this.stopBtn.disabled = true;
    await api(`/api/tasks/${this.id}/stop`, { method: "POST" });
    loadTasks();
  }

  /** The machine is gone: its terminals close and its outcome (and diff) take the stage. */
  ended(t) {
    for (const term of Object.values(this.terms)) term.dispose();
    this.terms = {};
    this.screen.hidden = true;
    if (this.resultFor === t.status.state) return;
    this.resultFor = t.status.state;
    this.result.hidden = false;
    this.result.replaceChildren(outcome(t), h("div", { class: "row-actions" },
      h("button", { class: "btn ghost small", type: "button", onclick: () => this.remove() }, "Remove from the list")));
    if (t.status.state === "done") appendDiff(t, this.result);
    // A failure explains itself at once: no need to look for the button.
    if (t.status.state === "failed") this.diag.show();
    // A closed machine's conversation is what is left of its work.
    this.conv.show(true, false);
  }

  async remove() {
    const r = await api(`/api/tasks/${this.id}`, { method: "DELETE" });
    if (r.ok) { toast("Removed from the list"); location.hash = "#/wall"; loadTasks(); } else toast(r.data?.error || "Could not remove", "err");
  }

  destroy() {
    for (const term of Object.values(this.terms)) term.dispose();
    this.tele.destroy();
    this.conv.destroy();
  }
}
