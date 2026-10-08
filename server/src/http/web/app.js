"use strict";
/* agentvm dashboard: a wall of live machines, a focused machine view, settings. */

const TERMINAL = new Set(["done", "no_changes", "failed", "stopped"]);
const $ = (s, r = document) => r.querySelector(s);
function h(tag, attrs = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    if (k === "class") e.className = v;
    else if (k === "style") e.style.cssText = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else e.setAttribute(k, v === true ? "" : v);
  }
  for (const kid of kids.flat()) if (kid != null && kid !== false) e.append(kid);
  return e;
}
function svg(tag, attrs = {}, ...kids) {
  const e = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  for (const kid of kids) e.append(kid);
  return e;
}
async function api(path, opts = {}) {
  const init = { ...opts, headers: { "content-type": "application/json", ...(opts.headers || {}) } };
  if (opts.body && typeof opts.body !== "string") init.body = JSON.stringify(opts.body);
  try {
    const res = await fetch(path, init);
    const text = await res.text();
    let data = null;
    try { data = text ? JSON.parse(text) : null; } catch { data = text; }
    return { ok: res.ok, status: res.status, data };
  } catch (e) {
    return { ok: false, status: 0, data: { error: "The agentvm server is not reachable." } };
  }
}

// ---------- formatting ----------
const repoName = p => p.split("/").filter(Boolean).pop() || p;
const shortPath = p => p.replace(/^\/Users\/[^/]+/, "~");
const plural = (n, one, many = one + "s") => `${n} ${n === 1 ? one : many}`;
function gb(mb) { return mb >= 1024 ? `${(mb / 1024).toFixed(mb >= 10240 ? 0 : 1)} GB` : `${Math.round(mb)} MB`; }
function rate(bps) {
  if (bps < 1024) return `${bps} B/s`;
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${(bps / 1024 / 1024).toFixed(1)} MB/s`;
}
function duration(s) {
  s = Math.max(0, Math.round(s));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}
const age = t => duration((t.finished_at ?? Date.now() / 1000) - t.created_at);
const titleOf = t => (t.prompt ? t.prompt.split("\n")[0] : repoName(t.repo));

/** Visual state of a machine: [css class, label]. */
function kind(t) {
  const s = t.status.state;
  if (s === "queued") return ["k-boot", "Queued"];
  if (s === "preparing" || s === "booting") return ["k-boot", "Booting"];
  if (s === "collecting") return ["k-boot", "Saving"];
  if (s === "running") {
    if (!t.interactive || t.activity === "working") return ["k-work", "Working"];
    if (t.activity === "waiting") return ["k-wait", "Waiting for you"];
    return ["k-idle", "Ready"];
  }
  if (s === "done") return ["k-done", `Done, ${plural(t.status.commits, "commit")}`];
  if (s === "no_changes") return ["k-done", "No changes"];
  if (s === "stopped") return ["k-stop", "Stopped"];
  return ["k-fail", "Failed"];
}

function sparkline(values, { width = 120, height = 26, max = 100, color = "var(--brand)" } = {}) {
  const pts = values.length ? values : [0];
  const step = width / Math.max(pts.length - 1, 1);
  const y = v => height - 1 - (Math.min(v, max) / max) * (height - 2);
  const line = pts.map((v, i) => `${i ? "L" : "M"}${(i * step).toFixed(1)},${y(v).toFixed(1)}`).join("");
  const area = `${line}L${((pts.length - 1) * step).toFixed(1)},${height}L0,${height}Z`;
  return svg("svg", { class: "spark", width, height, viewBox: `0 0 ${width} ${height}`, style: `--sc:${color}`, "aria-hidden": "true" },
    svg("path", { class: "area", d: area }), svg("path", { class: "line", d: line }));
}

function copyRow(text) {
  const btn = h("button", { class: "btn small", type: "button" }, "Copy");
  btn.addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(text); btn.textContent = "Copied"; } catch { btn.textContent = "Select it"; }
    setTimeout(() => (btn.textContent = "Copy"), 1500);
  });
  return h("div", { class: "cmd" }, h("code", {}, text), btn);
}

// ---------- shared state ----------
const state = { tasks: [], status: null, settings: null };
const byId = id => state.tasks.find(t => t.id === id);

const lastActivity = new Map();
function notifyWaiting(tasks) {
  for (const t of tasks) {
    const before = lastActivity.get(t.id);
    lastActivity.set(t.id, t.activity);
    if (before === "working" && t.activity === "waiting" && document.hidden && notificationsOn()) {
      const n = new Notification("Claude is waiting for you", { body: titleOf(t), icon: "/logo.svg", tag: t.id });
      n.onclick = () => { window.focus(); location.hash = `#/vm/${t.id}`; n.close(); };
    }
  }
}
const notificationsOn = () => "Notification" in window && Notification.permission === "granted" && localStorage.getItem("agentvm.notify") !== "off";

async function loadTasks() {
  const r = await api("/api/tasks");
  if (!r.ok) return;
  notifyWaiting(r.data);
  state.tasks = r.data.sort((a, b) => (TERMINAL.has(a.status.state) - TERMINAL.has(b.status.state)) || a.created_at - b.created_at);
  const waiting = state.tasks.filter(t => t.status.state === "running" && t.activity === "waiting").length;
  document.title = waiting ? `(${waiting}) agentvm` : "agentvm";
  current?.update();
}
async function loadStatus() {
  const r = await api("/api/status");
  if (r.ok) state.status = r.data;
  renderHost();
  updateLaunch();
}
async function loadSettings() {
  const r = await api("/api/settings");
  if (r.ok) state.settings = r.data;
}

// ---------- top bar: this Mac ----------
function gauge(label, value, pct, hot) {
  return h("div", { class: `gauge${hot ? " hot" : ""}` },
    h("div", { class: "row" }, h("span", {}, label), h("b", {}, value)),
    h("div", { class: "track" }, h("i", { style: `width:${Math.min(100, Math.max(0, pct))}%` })));
}
function renderHost() {
  const s = state.status;
  const box = $("#host");
  if (!s) return;
  const live = state.tasks.filter(t => t.metrics && t.status.state === "running");
  const cpu = live.length ? live.reduce((n, t) => n + t.metrics.cpu_pct * t.metrics.cpus, 0) / s.host.cpus : 0;
  const kids = [
    gauge("VMs", `${s.running} / ${s.concurrency}`, (100 * s.running) / s.concurrency, s.running >= s.concurrency),
    gauge("RAM reserved", `${gb(s.ram_committed_mb)} / ${gb(s.host.ram_mb)}`, (100 * s.ram_committed_mb) / s.host.ram_mb, s.ram_committed_mb > s.host.ram_mb * 0.85),
    gauge("CPU in VMs", `${cpu.toFixed(0)}%`, cpu, cpu > 85),
  ];
  if (!s.golden) kids.unshift(h("button", { class: "pill-warn", type: "button", onclick: () => (location.hash = "#/settings") }, "VM image missing"));
  if (!s.token) kids.unshift(h("button", { class: "pill-warn", type: "button", onclick: () => (location.hash = "#/settings") }, "Claude token missing"));
  box.replaceChildren(...kids);
}

// ---------- launcher ----------
const RECENT_KEY = "agentvm.recentRepos";
const repoEl = $("#repo"), promptEl = $("#prompt");
function recentRepos() {
  let saved = [];
  try { saved = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]"); } catch {}
  const preferred = state.settings?.settings.default_repo ? [state.settings.settings.default_repo] : [];
  return [...new Set([...preferred, ...saved, ...state.tasks.map(t => shortPath(t.repo))])].slice(0, 8);
}
function rememberRepo(p) {
  try { localStorage.setItem(RECENT_KEY, JSON.stringify([p, ...recentRepos().filter(x => x !== p)].slice(0, 8))); } catch {}
}
function showSuggest(open) {
  const box = $("#suggest");
  const q = repoEl.value.trim().toLowerCase();
  const list = recentRepos().filter(p => !q || (p.toLowerCase().includes(q) && p.toLowerCase() !== q));
  if (!open || !list.length) { box.hidden = true; return; }
  box.replaceChildren(...list.map(p => h("button", { type: "button", onmousedown: e => { e.preventDefault(); repoEl.value = p; box.hidden = true; updateLaunch(); promptEl.focus(); } },
    h("span", {}, repoName(p)), h("small", {}, p))));
  box.hidden = false;
}
repoEl.addEventListener("focus", () => showSuggest(true));
repoEl.addEventListener("input", () => { showSuggest(true); updateLaunch(); });
repoEl.addEventListener("blur", () => setTimeout(() => showSuggest(false), 120));
function prompts() {
  const text = promptEl.value.trim();
  if (!$("#batch").checked) return [text];
  const lines = text.split("\n").map(l => l.replace(/^\s*[-*•\d.)]+\s+/, "").trim()).filter(Boolean);
  return lines.length ? lines : [""];
}
function updateLaunch() {
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
const dialog = $("#launch-dialog");
function openLauncher() {
  if (dialog.open) return;
  dialog.showModal();
  (repoEl.value ? promptEl : repoEl).focus();
}
$("#new-vm").addEventListener("click", openLauncher);
document.addEventListener("keydown", e => {
  if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); openLauncher(); }
});
dialog.addEventListener("click", e => { if (e.target === dialog) dialog.close(); });
promptEl.addEventListener("input", () => { autosize(); updateLaunch(); });
$("#batch").addEventListener("change", updateLaunch);
$("#launcher").addEventListener("keydown", e => {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); $("#launcher").requestSubmit(); }
});
$("#launcher").addEventListener("submit", async e => {
  e.preventDefault();
  const repo_path = repoEl.value.trim();
  const model = $("#model").value || null;
  const err = $("#form-error");
  err.hidden = true;
  $("#launch").disabled = true;
  const ids = [];
  for (const prompt of prompts()) {
    const r = await api("/api/tasks", { method: "POST", body: { repo_path, prompt, interactive: true, model } });
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
});

// ---------- terminal attached to a VM ----------
const THEME = {
  background: "#090a0e", foreground: "#e3e7ee", cursor: "#a08cff", cursorAccent: "#090a0e", selectionBackground: "#3a3260",
  black: "#151821", red: "#ff6b6b", green: "#4fd18b", yellow: "#ffb547", blue: "#7aa7ff", magenta: "#b9a8ff", cyan: "#5fd7d7", white: "#d5dae3",
  brightBlack: "#5c6577", brightRed: "#ff8a8a", brightGreen: "#74e0a5", brightYellow: "#ffc977", brightBlue: "#9cbcff", brightMagenta: "#cfc2ff", brightCyan: "#86e3e3", brightWhite: "#ffffff",
};
class VmTerminal {
  constructor(id, session, { fontSize = 13, webgl = true, readOnly = false } = {}) {
    this.id = id;
    this.session = session;
    this.el = h("div", { class: "term" });
    this.xterm = new Terminal({
      fontFamily: '"Geist Mono", "SF Mono", ui-monospace, Menlo, monospace', fontSize, lineHeight: 1.15,
      cursorBlink: !readOnly, disableStdin: readOnly, macOptionIsMeta: true, macOptionClickForcesSelection: true,
      scrollback: 5000, theme: THEME,
    });
    this.fitter = new FitAddon.FitAddon();
    this.xterm.loadAddon(this.fitter);
    this.xterm.open(this.el);
    if (webgl) {
      try { const gl = new WebglAddon.WebglAddon(); gl.onContextLoss(() => gl.dispose()); this.xterm.loadAddon(gl); } catch {}
    }
    const enc = new TextEncoder();
    if (!readOnly) this.xterm.onData(d => { if (this.ws?.readyState === 1) this.ws.send(enc.encode(d)); });
    this.xterm.onResize(({ cols, rows }) => { if (this.ws?.readyState === 1) this.ws.send(JSON.stringify({ cols, rows })); });
    this.resizer = new ResizeObserver(() => this.fitSoon());
    this.resizer.observe(this.el);
  }
  fitSoon() {
    if (this.pending) return;
    this.pending = true;
    requestAnimationFrame(() => { this.pending = false; this.fit(); });
  }
  fit() {
    if (!this.el.isConnected || !this.el.offsetWidth) return;
    const d = this.fitter.proposeDimensions();
    if (d && (d.cols !== this.xterm.cols || d.rows !== this.xterm.rows)) this.xterm.resize(d.cols, d.rows);
  }
  connect() {
    if (this.ws || this.disposed) return;
    this.fit();
    if (this.xterm.cols < 20) this.xterm.resize(100, 30);
    const { cols, rows } = this.xterm;
    const ws = new WebSocket(`ws://${location.host}/api/tasks/${this.id}/pty?session=${this.session}&cols=${cols}&rows=${rows}`);
    ws.binaryType = "arraybuffer";
    // Resizes sent while connecting are lost: send the real size once the socket is open.
    ws.onopen = () => { this.fit(); ws.send(JSON.stringify({ cols: this.xterm.cols, rows: this.xterm.rows })); };
    ws.onmessage = e => this.xterm.write(new Uint8Array(e.data));
    ws.onclose = () => {
      if (this.ws !== ws) return;
      this.ws = null;
      if (!this.disposed && byId(this.id)?.status.state === "running") setTimeout(() => this.connect(), 900);
    };
    this.ws = ws;
  }
  dispose() {
    this.disposed = true;
    this.resizer.disconnect();
    this.ws?.close();
    this.xterm.dispose();
    this.el.remove();
  }
}

// ---------- outcome + diff ----------
function explain(reason = "") {
  if (reason === "timeout") return ["Time limit reached", "The agent was stopped after the time limit in Settings."];
  if (reason === "guest_no_result") return ["The VM shut down without a result", "Check console.log in the job folder."];
  if (reason.startsWith("claude_exit")) return ["Claude stopped with an error", ""];
  if (reason.startsWith("vm_error")) return ["The VM did not start", "Virtualization.framework reported an error."];
  if (reason.startsWith("fetch_failed")) return ["Could not import the branch", "The work finished but git fetch failed."];
  return ["Failed", ""];
}
function outcome(t) {
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
async function appendDiff(t, container) {
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

// ---------- views ----------
let current = null;
function mount(view) {
  current?.destroy();
  current = view;
  $("#view").replaceChildren(view.root);
  view.update();
}

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

/** The wall: every machine at once, live. */
class WallView {
  constructor() {
    this.cells = new Map();
    this.grid = h("div", { class: "wall" });
    this.head = h("div", { class: "wall-head" });
    this.root = h("div", { style: "flex:1;min-height:0;display:flex;flex-direction:column" }, this.head, this.grid);
  }
  update() {
    const tasks = state.tasks;
    if (!tasks.length) { this.root.replaceChildren(emptyState()); this.cells.clear(); return; }
    if (!this.grid.isConnected) this.root.replaceChildren(this.head, this.grid);
    const live = tasks.filter(t => !TERMINAL.has(t.status.state)).length;
    const waiting = tasks.filter(t => t.status.state === "running" && t.activity === "waiting").length;
    this.head.replaceChildren(h("b", {}, plural(live, "machine")), h("span", {}, waiting ? `${waiting} waiting for you` : "click a machine to work in it"));
    for (const [id, cell] of this.cells) if (!byId(id)) { cell.destroy(); this.cells.delete(id); }
    tasks.forEach((t, i) => {
      let cell = this.cells.get(t.id);
      if (!cell) { cell = new Cell(t); this.cells.set(t.id, cell); }
      if (this.grid.children[i] !== cell.root) this.grid.insertBefore(cell.root, this.grid.children[i] ?? null);
      cell.update(t);
    });
  }
  destroy() { for (const c of this.cells.values()) c.destroy(); }
}

class Cell {
  constructor(t) {
    this.id = t.id;
    this.state = h("span", { class: "state" });
    this.title = h("span", { class: "title" });
    this.body = h("div", { class: "cell-body", onclick: () => (location.hash = `#/vm/${t.id}`), title: "Open this machine" });
    this.cpu = h("span", { class: "metric" });
    this.mem = h("span", { class: "metric" });
    this.proc = h("span", { class: "proc" });
    this.root = h("article", { class: "cell" },
      h("div", { class: "cell-head" }, h("span", { class: "dot" }), this.title, this.state),
      this.body,
      h("div", { class: "cell-foot" }, this.cpu, this.mem, this.proc));
  }
  update(t) {
    const [cls, label] = kind(t);
    this.root.className = `cell ${cls}${TERMINAL.has(t.status.state) ? " ended-cell" : ""}`;
    this.title.textContent = titleOf(t);
    this.title.title = `${titleOf(t)}\n${t.repo}`;
    this.state.textContent = `${label}, ${age(t)}`;
    const m = t.metrics;
    this.cpu.replaceChildren(h("b", {}, m ? `${m.cpu_pct.toFixed(0)}%` : "—"), "CPU", sparkline(t.cpu_history, { width: 70, height: 16 }));
    this.mem.replaceChildren(h("b", {}, m ? gb(m.mem_used_mb) : "—"), "RAM", sparkline(t.mem_history, { width: 70, height: 16, color: "var(--wait)" }));
    this.proc.textContent = m?.top?.[0] ? `${m.top[0].name} ${m.top[0].cpu_pct.toFixed(0)}%` : repoName(t.repo);
    if (t.status.state === "running" && t.interactive) {
      if (!this.term) {
        this.body.replaceChildren();
        this.term = new VmTerminal(t.id, "claude", { fontSize: 10.5, webgl: false, readOnly: true });
        this.body.append(this.term.el);
      }
      this.term.connect();
    } else {
      this.term?.dispose();
      this.term = null;
      const text = TERMINAL.has(t.status.state)
        ? [h("b", {}, label), h("span", {}, repoName(t.repo))]
        : [h("b", {}, label), h("span", {}, t.interactive ? "Debian boots in a few seconds, then Claude opens." : "Running without a terminal.")];
      if (this.endedFor !== label) { this.endedFor = label; this.body.replaceChildren(h("div", { class: "ended" }, ...text)); }
    }
  }
  destroy() { this.term?.dispose(); }
}

/** One machine, full size, with its telemetry. */
class FocusView {
  constructor(id) {
    this.id = id;
    this.session = "claude";
    this.terms = {};
    this.showPanel = localStorage.getItem("agentvm.panel") !== "off";
    this.rail = h("nav", { class: "rail", "aria-label": "Machines" });
    this.titleEl = h("span", { class: "title" });
    this.stateEl = h("span", { class: "status" });
    this.note = h("span", { class: "note" });
    this.segBtns = ["claude", "shell"].map(s => h("button", { type: "button", "aria-pressed": String(s === "claude"), onclick: () => this.show(s) }, s === "claude" ? "Claude" : "Root shell"));
    this.saveBtn = h("button", { class: "btn", type: "button", onclick: () => this.save() }, "Save to repo");
    this.closeBtn = h("button", { class: "btn", type: "button", onclick: () => this.close() }, "Close VM");
    this.stopBtn = h("button", { class: "btn ghost danger", type: "button", title: "Power off now without saving", onclick: () => this.stop() }, "Force stop");
    this.panelBtn = h("button", { class: "btn ghost", type: "button", onclick: () => this.togglePanel() }, "Telemetry");
    this.seg = h("div", { class: "seg" }, this.segBtns);
    this.toolbar = h("div", { class: "toolbar" }, h("span", { class: "dot" }), this.titleEl, this.seg, h("span", { class: "spacer" }), this.note, this.saveBtn, this.closeBtn, this.stopBtn, this.panelBtn);
    this.overlay = h("div", { class: "overlay" });
    this.screen = h("div", { class: "screen" }, this.overlay);
    this.result = h("div", { class: "result" });
    this.result.hidden = true;
    this.stage = h("section", { class: "stage" }, this.toolbar, this.screen, this.result);
    this.panel = h("aside", { class: "panel", "aria-label": "Telemetry" });
    this.root = h("div", { class: "focus" }, this.rail, this.stage, this.panel);
  }

  update() {
    const t = byId(this.id);
    this.renderRail();
    if (!t) { this.overlayText("Machine not found", "It may belong to a previous server session."); return; }
    const [cls, label] = kind(t);
    this.root.className = `focus ${cls}${this.showPanel ? "" : " no-panel"}`;
    this.panel.hidden = !this.showPanel;
    this.toolbar.className = `toolbar ${cls}`;
    this.titleEl.textContent = titleOf(t);
    this.titleEl.title = `${t.prompt || "(no first task)"}\n${t.repo}`;
    const s = t.status.state;
    const ended = TERMINAL.has(s);
    for (const b of [this.saveBtn, this.closeBtn, this.seg]) b.hidden = !t.interactive || ended;
    this.saveBtn.disabled = this.closeBtn.disabled = s !== "running";
    this.stopBtn.hidden = ended;
    this.renderPanel(t, label);
    if (ended) return this.ended(t);
    if (!t.interactive) return this.overlayText("Running without a terminal", "Started from the API in automatic mode.");
    if (s !== "running") return this.overlayText(s === "queued" ? "Queued" : "Booting the VM", s === "queued" ? "Every VM slot is busy; it starts as soon as one frees up." : "Debian is starting. Claude opens on its own in a few seconds.");
    this.overlay.hidden = true;
    this.ensureTerm(this.session).connect();
  }

  renderRail() {
    const items = state.tasks.map(t => {
      const [cls, label] = kind(t);
      return h("button", { class: `rail-item ${cls}`, type: "button", "aria-current": String(t.id === this.id), onclick: () => (location.hash = `#/vm/${t.id}`) },
        h("span", { class: "dot" }), h("span", { class: "t" }, titleOf(t)), h("span", { class: "s" }, label));
    });
    this.rail.replaceChildren(h("a", { class: "rail-back", href: "#/wall" }, "← All machines"), ...items);
  }

  renderPanel(t, label) {
    if (!this.showPanel) return;
    const m = t.metrics;
    const kids = [];
    if (m) {
      kids.push(
        h("div", { class: "stat" }, h("h3", {}, "CPU"), h("div", { class: "big" }, `${m.cpu_pct.toFixed(0)}%`, h("small", {}, `of ${m.cpus} vCPUs`)), sparkline(t.cpu_history, { width: 250, height: 40 })),
        h("div", { class: "stat" }, h("h3", {}, "Memory"), h("div", { class: "big" }, gb(m.mem_used_mb), h("small", {}, `of ${gb(m.mem_total_mb)}`)), sparkline(t.mem_history, { width: 250, height: 40, color: "var(--wait)" })),
        h("div", { class: "stat" }, h("h3", {}, "Disk"), h("div", {}, `${gb(m.disk_used_mb)} of ${gb(m.disk_total_mb)}`),
          h("div", { class: "bar-line" }, h("i", { style: `width:${(100 * m.disk_used_mb) / Math.max(m.disk_total_mb, 1)}%` }))),
        h("div", {}, h("h3", {}, "Busiest processes"), m.top.length
          ? h("div", { class: "procs" }, m.top.map(p => h("div", {}, h("span", {}, p.name), h("span", {}, `${p.cpu_pct.toFixed(0)}%`), h("span", {}, gb(p.mem_mb)))))
          : h("div", { class: "procs" }, h("span", {}, "Idle"))),
        h("dl", { class: "kv" },
          h("dt", {}, "Network in"), h("dd", {}, rate(m.net_rx_bps)),
          h("dt", {}, "Network out"), h("dd", {}, rate(m.net_tx_bps)),
          h("dt", {}, "Load"), h("dd", {}, m.load1.toFixed(2)),
          h("dt", {}, "Processes"), h("dd", {}, String(m.procs)),
          h("dt", {}, "VM uptime"), h("dd", {}, duration(m.uptime_s))));
    }
    kids.push(h("dl", { class: "kv" },
      h("dt", {}, "State"), h("dd", {}, label),
      h("dt", {}, "Model"), h("dd", {}, t.model === "default" ? "Default" : t.model[0].toUpperCase() + t.model.slice(1)),
      h("dt", {}, "Repository"), h("dd", { title: t.repo }, repoName(t.repo)),
      h("dt", {}, "Branch"), h("dd", { title: t.branch }, t.branch),
      h("dt", {}, "From commit"), h("dd", {}, t.base_sha.slice(0, 10)),
      h("dt", {}, "Age"), h("dd", {}, age(t))));
    this.panel.replaceChildren(...kids);
  }

  overlayText(title, text) {
    this.screen.hidden = false;
    this.result.hidden = true;
    this.overlay.hidden = false;
    this.overlay.replaceChildren(h("div", {}, h("img", { class: "mark", src: "/logo.svg", alt: "" }), h("b", {}, title), text));
  }

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

  async save() {
    this.saveBtn.disabled = true;
    this.note.style.color = "";
    this.note.textContent = "Saving…";
    const r = await api(`/api/tasks/${this.id}/save`, { method: "POST" });
    this.note.textContent = r.ok ? (r.data.commits ? `Saved: ${plural(r.data.commits, "commit")} on agent/${this.id}` : "Nothing to save yet") : r.data?.error || "Save failed";
    if (!r.ok) this.note.style.color = "var(--fail)";
    this.saveBtn.disabled = false;
    this.terms[this.session]?.xterm.focus();
  }
  async close() {
    this.closeBtn.disabled = this.saveBtn.disabled = true;
    this.closeBtn.textContent = "Closing…";
    await api(`/api/tasks/${this.id}/close`, { method: "POST" });
    loadTasks();
  }
  async stop() {
    this.stopBtn.disabled = true;
    await api(`/api/tasks/${this.id}/stop`, { method: "POST" });
    loadTasks();
  }

  ended(t) {
    for (const term of Object.values(this.terms)) term.dispose();
    this.terms = {};
    this.screen.hidden = true;
    this.note.textContent = "";
    if (this.resultFor === t.status.state) return;
    this.resultFor = t.status.state;
    this.result.hidden = false;
    this.result.replaceChildren(outcome(t));
    if (t.status.state === "done") appendDiff(t, this.result);
  }

  destroy() { for (const term of Object.values(this.terms)) term.dispose(); }
}

/** Settings: full width, section index on the left, every change saved on the spot. */
class SettingsView {
  constructor() {
    this.root = h("div", { class: "settings" });
    this.built = false;
    this.saveTimer = null;
  }

  /** Built once; live parts refresh on actions and while the image is rebuilding. */
  async update() {
    if (this.built) return;
    this.built = true;
    await loadSettings();
    this.draft = { ...state.settings.settings };
    const lim = state.settings.limits;
    const sections = [
      ["resources", "Resources", this.resources(lim)],
      ["agent", "Agent", this.agent()],
      ["account", "Claude account", this.account()],
      ["notifications", "Notifications", this.notifications()],
      ["image", "VM image", this.image()],
      ["storage", "Storage", this.storage()],
    ];
    this.navLinks = sections.map(([id, label]) => h("a", { href: `#/settings`, "data-target": id, onclick: e => { e.preventDefault(); this.scrollTo(id); } }, label));
    this.content = h("div", { class: "settings-content" },
      h("header", { class: "settings-head" }, h("h1", {}, "Settings"), this.saveState = h("span", { class: "save-state" }, "Changes are saved automatically")),
      h("div", { class: "settings-grid" }, sections.map(([id, label, body]) => h("section", { class: `card span-${id}`, id: `set-${id}`, "data-section": id }, h("h2", {}, label), body))));
    this.root.replaceChildren(h("nav", { class: "settings-nav", "aria-label": "Settings sections" }, this.navLinks), this.content);
    this.content.addEventListener("scroll", () => this.spy());
    this.spy();
    this.refreshLive();
  }

  scrollTo(id) {
    $(`#set-${id}`, this.root)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }
  spy() {
    const top = this.content.getBoundingClientRect().top + 80;
    let active = "resources";
    for (const sec of this.content.querySelectorAll("[data-section]")) if (sec.getBoundingClientRect().top <= top) active = sec.dataset.section;
    for (const a of this.navLinks) a.setAttribute("aria-current", String(a.dataset.target === active));
  }

  // ----- sections -----
  resources(lim) {
    const s = this.draft;
    const memChoices = [1024, 2048, 4096, 6144, 8192, 12288, 16384].filter(v => v <= lim.ram_mb - 8192);
    this.vmsOut = h("output", { class: "big-value" });
    this.cpuOut = h("output", { class: "big-value" });
    this.vmsInput = h("input", { type: "range", min: "1", max: String(Math.min(64, Math.max(lim.cpus * 2, 16))), value: String(s.max_vms), oninput: e => this.set("max_vms", Number(e.target.value)) });
    this.cpuInput = h("input", { type: "range", min: "1", max: String(lim.cpus), value: String(s.cpus), oninput: e => this.set("cpus", Number(e.target.value)) });
    this.memSeg = h("div", { class: "seg wide", role: "radiogroup", "aria-label": "Memory per VM" },
      memChoices.map(v => h("button", { type: "button", role: "radio", "data-value": String(v), onclick: () => this.set("memory_mb", v) }, gb(v))));
    this.budget = h("div", { class: "budget" });
    this.resErr = h("p", { class: "field-error", role: "alert" });
    const block = h("div", { class: "res" },
      h("div", { class: "slider" }, h("div", { class: "slider-head" }, h("label", {}, "VMs at the same time"), this.vmsOut), this.vmsInput,
        h("p", { class: "hint" }, "Extra launches wait in a queue. Takes effect at once.")),
      h("div", { class: "slider" }, h("div", { class: "slider-head" }, h("label", {}, "vCPUs per VM"), this.cpuOut), this.cpuInput,
        h("p", { class: "hint" }, `This Mac has ${lim.cpus} cores. vCPUs are shared, so the total can exceed them.`)),
      h("div", { class: "slider" }, h("div", { class: "slider-head" }, h("label", {}, "Memory per VM")), this.memSeg,
        h("p", { class: "hint" }, "Reserved for each running VM. New VMs use the new size.")));
    this.paintResources();
    return [h("p", { class: "lede" }, `Resources of this Mac given to the VMs: ${lim.cpus} cores, ${gb(lim.ram_mb)} of memory.`), block, this.budget, this.resErr];
  }

  paintResources() {
    const s = this.draft, lim = state.settings.limits;
    this.vmsOut.textContent = String(s.max_vms);
    this.cpuOut.textContent = String(s.cpus);
    for (const b of this.memSeg.children) b.setAttribute("aria-checked", String(Number(b.dataset.value) === s.memory_mb));
    const total = lim.ram_mb, reserve = 8192, used = s.max_vms * s.memory_mb, fit = Math.max(1, Math.floor((total - reserve) / s.memory_mb));
    const over = used > total - reserve;
    this.budget.className = `budget${over ? " over" : ""}`;
    this.budget.replaceChildren(
      h("div", { class: "budget-bar" },
        h("i", { class: "vms", style: `width:${Math.min(100, (100 * used) / total)}%` }),
        h("i", { class: "reserve", style: `width:${(100 * reserve) / total}%` })),
      h("div", { class: "budget-legend" },
        h("span", {}, h("b", {}, `${s.max_vms} × ${gb(s.memory_mb)} = ${gb(used)}`), ` for VMs at full load`),
        h("span", {}, `${gb(reserve)} kept for macOS`),
        h("span", { class: "fit" }, over ? `Over budget: at most ${fit} VMs of ${gb(s.memory_mb)} fit without swapping` : `Fits: up to ${fit} VMs of ${gb(s.memory_mb)}`)));
  }

  agent() {
    const s = this.draft;
    const models = [["default", "Account default", "Whatever Claude Code picks"], ["sonnet", "Sonnet", "Fast, great for most work"], ["opus", "Opus", "Most capable, slower"], ["haiku", "Haiku", "Fastest and lightest"]];
    this.modelCards = h("div", { class: "choices", role: "radiogroup", "aria-label": "Default model" },
      models.map(([v, name, desc]) => h("button", { type: "button", role: "radio", "data-value": v, "aria-checked": String(s.model === v), onclick: () => this.set("model", v) }, h("b", {}, name), h("span", {}, desc))));
    this.timeoutIn = h("input", { type: "number", min: "1", max: "1440", value: String(Math.round(s.timeout_s / 60)), oninput: e => this.set("timeout_s", Number(e.target.value) * 60) });
    this.repoIn = h("input", { value: s.default_repo ?? "", placeholder: "~/code/my-app", spellcheck: "false", oninput: e => this.set("default_repo", e.target.value.trim() || null) });
    this.agentErr = h("p", { class: "field-error", role: "alert" });
    return [
      h("div", { class: "set" }, h("label", {}, "Default model"), this.modelCards, h("p", { class: "hint" }, "Every launch can pick a different one.")),
      h("div", { class: "pair" },
        h("div", { class: "set" }, h("label", {}, "Time limit for automatic tasks"), h("div", { class: "unit" }, this.timeoutIn, h("span", {}, "minutes")), h("p", { class: "hint" }, "Terminals never time out: you close them.")),
        h("div", { class: "set" }, h("label", {}, "Default repository"), this.repoIn, h("p", { class: "hint" }, "Prefilled in New VM."))),
      this.agentErr];
  }

  account() {
    this.tokenPill = h("span", { class: "pill" });
    this.tokenMsg = h("span", { class: "msg" });
    const input = h("input", { name: "token", type: "password", placeholder: "Paste a token: sk-ant-oat01-…", autocomplete: "off", spellcheck: "false" });
    const form = h("form", { class: "inline-form", onsubmit: e => this.saveToken(e, input) }, input, h("button", { class: "btn", type: "submit" }, "Save token"));
    return [
      h("div", { class: "status-line" }, this.tokenPill, h("span", { class: "lede" }, "A long-lived token of your Claude subscription, stored in the macOS Keychain.")),
      form, this.tokenMsg,
      h("p", { class: "hint" }, "Create one in a terminal with ", h("code", {}, "claude setup-token"), ". Running VMs keep the token they started with.")];
  }

  notifications() {
    this.notifySwitch = h("button", { class: "switch", type: "button", role: "switch", onclick: () => this.toggleNotify() }, h("i"));
    this.notifyMsg = h("span", { class: "hint" });
    this.showNotify();
    return [h("div", { class: "status-line" }, this.notifySwitch, h("div", {}, h("b", {}, "Notify me when Claude is waiting"),
      h("p", { class: "hint" }, "A desktop notification when an agent finishes a turn while this page is in the background.")), this.notifyMsg)];
  }

  image() {
    this.goldenFacts = h("div", { class: "facts" });
    this.goldenLog = h("pre", { class: "log", hidden: true });
    this.rebuildBtn = h("button", { class: "btn", type: "button", onclick: () => this.rebuild() }, "Rebuild image");
    this.goldenMsg = h("span", { class: "msg" });
    return [h("p", { class: "lede" }, "Every VM starts as an instant copy of this Debian 13 image with Claude Code preinstalled. Rebuild it to update Claude Code and the system packages; running VMs are not affected."),
      this.goldenFacts, h("div", { class: "row-actions" }, this.rebuildBtn, this.goldenMsg), this.goldenLog];
  }

  storage() {
    this.storageFacts = h("div", { class: "facts" });
    this.cleanMsg = h("span", { class: "msg" });
    return [h("p", { class: "lede" }, "Disks of closed VMs are deleted at once. Their logs stay until you clean them up."),
      this.storageFacts, h("div", { class: "row-actions" }, h("button", { class: "btn", type: "button", onclick: () => this.cleanup() }, "Delete logs of closed VMs"), this.cleanMsg)];
  }

  // ----- auto-save -----
  set(key, value) {
    this.draft[key] = value;
    if (key === "model") for (const b of this.modelCards.children) b.setAttribute("aria-checked", String(b.dataset.value === value));
    this.paintResources();
    this.saveState.className = "save-state pending";
    this.saveState.textContent = "Saving…";
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => this.save(), 450);
  }
  async save() {
    const r = await api("/api/settings", { method: "PUT", body: this.draft });
    const err = r.ok ? "" : r.data?.error || "Could not save.";
    const resourceError = /VM|vCPU|memory/i.test(err);
    this.resErr.textContent = resourceError ? err : "";
    this.agentErr.textContent = err && !resourceError ? err : "";
    this.saveState.className = `save-state ${r.ok ? "ok" : "err"}`;
    this.saveState.textContent = r.ok ? "All changes saved" : "Not saved: fix the highlighted value";
    if (r.ok) { state.settings = r.data; loadStatus(); }
  }

  async saveToken(e, input) {
    e.preventDefault();
    const r = await api("/api/settings/token", { method: "PUT", body: { token: input.value } });
    this.tokenMsg.className = `msg ${r.ok ? "ok" : "err"}`;
    this.tokenMsg.textContent = r.ok ? "Saved in the Keychain. New VMs use it." : r.data?.error || "Could not save the token.";
    if (r.ok) { input.value = ""; await loadStatus(); this.paintToken(); }
  }
  paintToken() {
    const ok = state.status?.token;
    this.tokenPill.className = `pill ${ok ? "ok" : "err"}`;
    this.tokenPill.textContent = ok ? "Connected" : "Missing";
  }

  showNotify() {
    const on = notificationsOn();
    this.notifySwitch.setAttribute("aria-checked", String(on));
    this.notifyMsg.textContent = !("Notification" in window) ? "Not supported here." : Notification.permission === "denied" ? "Blocked in the browser settings." : "";
  }
  async toggleNotify() {
    if (!("Notification" in window)) return;
    if (notificationsOn()) localStorage.setItem("agentvm.notify", "off");
    else { localStorage.removeItem("agentvm.notify"); if (Notification.permission === "default") await Notification.requestPermission(); }
    this.showNotify();
  }

  async rebuild() {
    const r = await api("/api/golden/rebuild", { method: "POST" });
    this.goldenMsg.className = `msg ${r.ok ? "" : "err"}`;
    this.goldenMsg.textContent = r.ok ? "Rebuilding, about 2 minutes…" : r.data?.error || "Could not start the rebuild.";
    this.refreshLive();
  }
  async cleanup() {
    const r = await api("/api/storage/cleanup", { method: "POST" });
    this.cleanMsg.className = `msg ${r.ok ? "ok" : "err"}`;
    this.cleanMsg.textContent = r.ok ? `Deleted ${plural(r.data.removed, "job folder")}.` : "Cleanup failed.";
    this.refreshLive();
  }

  async refreshLive() {
    this.paintToken();
    const fact = (label, value) => h("div", { class: "fact" }, h("span", {}, label), h("b", {}, value));
    const [g, st] = await Promise.all([api("/api/golden"), api("/api/storage")]);
    if (g.ok) {
      const d = g.data;
      this.goldenFacts.replaceChildren(
        fact("Status", d.rebuilding ? "Rebuilding" : d.exists ? "Ready" : "Missing"),
        fact("Claude Code", d.claude_version ?? "unknown"),
        fact("Built", d.built_at ? new Date(d.built_at * 1000).toLocaleString() : "never"),
        fact("Size on disk", gb(d.size_mb)));
      this.rebuildBtn.disabled = d.rebuilding;
      this.goldenLog.hidden = !(d.rebuilding || d.last_result);
      this.goldenLog.textContent = d.log_tail;
      if (d.last_result && !d.rebuilding) { this.goldenMsg.className = `msg ${d.last_result === "ok" ? "ok" : "err"}`; this.goldenMsg.textContent = d.last_result === "ok" ? "Image rebuilt." : `Rebuild ${d.last_result}.`; }
      if (d.rebuilding) setTimeout(() => current === this && this.refreshLive(), 1500);
    }
    if (st.ok) this.storageFacts.replaceChildren(fact("Job folders", String(st.data.jobs)), fact("Job logs", gb(st.data.jobs_mb)), fact("VM image", gb(st.data.golden_mb)));
  }

  destroy() { clearTimeout(this.saveTimer); }
}

// ---------- routing ----------
function route() {
  const hash = location.hash || "#/wall";
  for (const a of document.querySelectorAll("[data-nav]")) a.setAttribute("aria-current", hash.startsWith(`#/${a.dataset.nav}`) || (a.dataset.nav === "wall" && hash.startsWith("#/vm/")) ? "page" : "false");
  const vm = hash.match(/^#\/vm\/(.+)$/);
  if (vm) {
    if (current instanceof FocusView && current.id === vm[1]) return;
    mount(new FocusView(vm[1]));
  } else if (hash.startsWith("#/settings")) {
    if (!(current instanceof SettingsView)) mount(new SettingsView());
  } else if (!(current instanceof WallView)) {
    mount(new WallView());
  }
}
window.addEventListener("hashchange", route);

(async () => {
  await Promise.all([loadSettings(), loadStatus()]);
  await loadTasks();
  repoEl.value = recentRepos()[0] ?? "";
  if (state.settings?.settings.model && state.settings.settings.model !== "default") $("#model").value = state.settings.settings.model;
  updateLaunch();
  route();
  setInterval(loadTasks, 1000);
  setInterval(loadStatus, 3000);
})();
