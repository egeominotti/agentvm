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
    // The cookie is gone or the token changed: the page at / explains how to get back in.
    if (res.status === 401) location.replace("/");
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
const money = n => (!n ? "$0" : n < 0.01 ? "<$0.01" : n < 1 ? `$${n.toFixed(3)}` : n < 100 ? `$${n.toFixed(2)}` : `$${n.toFixed(0)}`);
const tokens = n => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1000 ? `${(n / 1000).toFixed(n >= 1e4 ? 0 : 1)}k` : String(n));
function duration(s) {
  s = Math.max(0, Math.round(s));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}
const age = t => duration((t.finished_at ?? Date.now() / 1000) - t.created_at);
const titleOf = t => (t.prompt ? t.prompt.split("\n")[0] : t.label || repoName(t.repo));

/** Visual state of a machine: [css class, label]. */
function kind(t) {
  const s = t.status.state;
  if (s === "queued") return ["k-boot", "Queued"];
  if (s === "preparing" || s === "booting") return ["k-boot", "Booting"];
  if (s === "collecting") return ["k-boot", "Saving"];
  if (s === "running") {
    if (t.interactive && !t.ready) return ["k-boot", "Booting"];
    if (!t.interactive || t.activity === "working") return ["k-work", "Working"];
    if (t.activity === "waiting") return ["k-wait", "Waiting for you"];
    return ["k-idle", "Ready"];
  }
  if (s === "done") return ["k-done", `Done, ${plural(t.status.commits, "commit")}`];
  if (s === "no_changes") return ["k-done", "No changes"];
  if (s === "stopped") return ["k-stop", "Stopped"];
  return ["k-fail", "Failed"];
}

function sparkline(values, { width = 120, height = 26, max = 100, color = "var(--brand)", fluid = false } = {}) {
  const pts = values.length ? values : [0];
  const step = width / Math.max(pts.length - 1, 1);
  const y = v => height - 1 - (Math.min(v, max) / max) * (height - 2);
  const line = pts.map((v, i) => `${i ? "L" : "M"}${(i * step).toFixed(1)},${y(v).toFixed(1)}`).join("");
  const area = `${line}L${((pts.length - 1) * step).toFixed(1)},${height}L0,${height}Z`;
  return svg("svg", { class: "spark", width: fluid ? "100%" : width, height, viewBox: `0 0 ${width} ${height}`, preserveAspectRatio: "none", style: `--sc:${color}`, "aria-hidden": "true" },
    svg("path", { class: "area", d: area }), svg("path", { class: "line", d: line }));
}

/** Short message in the corner: never shifts the layout. */
function toast(text, kind = "ok") {
  let box = $("#toasts");
  if (!box) { box = h("div", { id: "toasts", class: "toasts", role: "status", "aria-live": "polite" }); document.body.append(box); }
  const item = h("div", { class: `toast ${kind}` }, text);
  box.append(item);
  setTimeout(() => { item.classList.add("out"); setTimeout(() => item.remove(), 300); }, kind === "err" ? 6000 : 3500);
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
  renderSide();
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

// ---------- sidebar: this Mac and the machines ----------
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
    gauge("Memory reserved", `${gb(s.ram_committed_mb)} / ${gb(s.host.ram_mb)}`, (100 * s.ram_committed_mb) / s.host.ram_mb, s.ram_committed_mb > s.host.ram_mb * 0.85),
    gauge("CPU in VMs", `${cpu.toFixed(0)}%`, cpu, cpu > 85),
  ];
  const spent = state.tasks.reduce((n, t) => n + (t.usage?.cost_usd ?? 0), 0);
  kids.push(h("div", { class: "gauge spend", title: "What these agents would cost at API prices; with a subscription it counts against its limits" },
    h("div", { class: "row" }, h("span", {}, "Claude usage"), h("b", {}, money(spent)))));
  box.replaceChildren(...kids);
  const alerts = [];
  if (!s.token) alerts.push(h("button", { class: "pill-warn", type: "button", onclick: () => (location.hash = "#/settings") }, "Claude token missing"));
  if (!s.golden) alerts.push(h("button", { class: "pill-warn", type: "button", onclick: () => (location.hash = "#/settings") }, "VM image missing"));
  $("#alerts").replaceChildren(...alerts);
}

/** Every machine in the sidebar, like a list of issues. Patched in place: a link replaced
 * while the mouse button is down would swallow the click. */
const sideItems = new Map();
function renderSide() {
  const focused = location.hash.match(/^#\/vm\/(.+)$/)?.[1];
  const live = state.tasks.filter(t => !TERMINAL.has(t.status.state)).length;
  const count = live ? String(live) : "";
  if ($("#count-machines").textContent !== count) $("#count-machines").textContent = count;
  // Running machines only: finished ones stay on the wall until cleared.
  const shown = state.tasks.filter(t => !TERMINAL.has(t.status.state) || t.id === focused);
  const box = $("#side-machines");
  for (const [id, el] of sideItems) if (!shown.some(t => t.id === id)) { el.remove(); sideItems.delete(id); }
  shown.forEach((t, i) => {
    const [cls, label] = kind(t);
    let el = sideItems.get(t.id);
    if (!el) {
      el = h("a", { href: `#/vm/${t.id}` }, h("span", { class: "dot" }), h("span", { class: "t" }), h("span", { class: "m" }));
      sideItems.set(t.id, el);
    }
    const set = (node, text) => { if (node.textContent !== text) node.textContent = text; };
    const className = `side-vm ${cls}`;
    if (el.className !== className) el.className = className;
    el.setAttribute("aria-current", String(t.id === focused));
    el.title = `${titleOf(t)}\n${label}`;
    set(el.children[1], titleOf(t));
    set(el.children[2], t.activity === "waiting" && t.status.state === "running" ? "waiting" : age(t));
    if (box.children[i] !== el) box.insertBefore(el, box.children[i] ?? null);
  });
  const empty = box.querySelector(".side-empty");
  if (!shown.length && !empty) box.append(h("p", { class: "side-empty" }, "No running machines"));
  if (shown.length && empty) empty.remove();
}

// ---------- models and Claude Code versions ----------
const MODELS = [
  ["default", "Account default", "Whatever Claude Code picks"],
  ["opus", "Opus", "Most capable"],
  ["sonnet", "Sonnet", "Fast, great for most work"],
  ["haiku", "Haiku", "Fastest and lightest"],
  ["fable", "Fable", "Fable family"],
  ["opusplan", "Opus plans, Sonnet builds", "Opus in plan mode, Sonnet otherwise"],
  ["opus[1m]", "Opus, 1M context", "Long context window"],
  ["sonnet[1m]", "Sonnet, 1M context", "Long context window"],
];
const modelLabel = id => (MODELS.find(m => m[0] === id)?.[1]) ?? id;
function fillModelSelect(select, current) {
  const known = MODELS.some(m => m[0] === current);
  select.replaceChildren(
    ...MODELS.map(([v, l]) => h("option", { value: v, selected: v === current }, l)),
    h("option", { value: "__custom", selected: !known && !!current }, "Other model ID…"));
}
let releases = null;
async function loadReleases() {
  if (releases) return releases;
  const r = await api("/api/claude/versions");
  if (r.ok) releases = r.data;
  return releases;
}
function versionOptions(includeImage, imageVersion, current) {
  const opts = [];
  if (includeImage) opts.push(h("option", { value: "" }, imageVersion ? `Image version (${imageVersion})` : "Image version"));
  if (releases) {
    opts.push(h("option", { value: "latest", selected: current === "latest" }, `Latest (${releases.latest})`));
    opts.push(h("option", { value: "stable", selected: current === "stable" }, `Stable (${releases.stable})`));
    opts.push(h("optgroup", { label: "Exact version" }, releases.versions.map(v => h("option", { value: v, selected: current === v }, v))));
  } else {
    for (const v of ["latest", "stable"]) opts.push(h("option", { value: v, selected: current === v }, v[0].toUpperCase() + v.slice(1)));
    if (current && !["latest", "stable", ""].includes(current)) opts.push(h("option", { value: current, selected: true }, current));
  }
  return opts;
}
function fillResourceSelects() {
  const set = state.settings;
  if (!set) return;
  const { cpus: hostCpus, ram_mb: hostRam } = set.limits;
  const cpuChoices = [...new Set([1, 2, 4, 6, 8, 12, 16, hostCpus].filter(n => n <= hostCpus))].sort((a, b) => a - b);
  const memChoices = [1024, 2048, 4096, 6144, 8192, 12288, 16384, 24576, 32768].filter(v => v <= hostRam - 8192);
  $("#vm-cpus").replaceChildren(...cpuChoices.map(n => h("option", { value: String(n), selected: n === set.settings.cpus }, String(n))));
  $("#vm-mem").replaceChildren(...memChoices.map(v => h("option", { value: String(v), selected: v === set.settings.memory_mb }, gb(v))));
  updateResourceHint();
}
function updateResourceHint() {
  const s = state.status, set = state.settings;
  if (!s || !set) return;
  const mem = Number($("#vm-mem").value || set.settings.memory_mb);
  const free = s.host.ram_mb - 8192 - s.ram_committed_mb;
  const n = prompts().length;
  const fits = Math.floor(free / mem);
  const hint = $("#res-hint");
  hint.className = `res-hint${fits < n ? " over" : ""}`;
  hint.textContent = fits < n
    ? `Only ${Math.max(fits, 0)} more VM${fits === 1 ? "" : "s"} of ${gb(mem)} fit in free memory; the rest will wait or swap.`
    : `${gb(Math.max(free, 0))} free for VMs: room for ${fits} like this.`;
}
$("#vm-mem").addEventListener("change", updateResourceHint);

async function fillLauncherChoices() {
  fillResourceSelects();
  fillModelSelect($("#model"), state.settings?.settings.model ?? "default");
  const [g] = await Promise.all([api("/api/golden"), loadReleases()]);
  $("#claude-version").replaceChildren(...versionOptions(true, g.ok ? g.data.claude_version : null, ""));
}
$("#model").addEventListener("change", e => {
  const custom = e.target.value === "__custom";
  $("#model-custom").hidden = !custom;
  if (custom) $("#model-custom").focus();
});
const chosenModel = () => ($("#model").value === "__custom" ? $("#model-custom").value.trim() : $("#model").value) || null;

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
  fillLauncherChoices();
  dialog.showModal();
  (repoEl.value ? promptEl : repoEl).focus();
}
$("#new-vm").addEventListener("click", openLauncher);
document.addEventListener("keydown", e => {
  if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); openLauncher(); }
});
dialog.addEventListener("click", e => { if (e.target === dialog) dialog.close(); });
$("#launch-close").addEventListener("click", () => dialog.close());
promptEl.addEventListener("input", () => { autosize(); updateLaunch(); updateResourceHint(); });
$("#batch").addEventListener("change", () => { updateLaunch(); updateResourceHint(); });
$("#launcher").addEventListener("keydown", e => {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); $("#launcher").requestSubmit(); }
});
$("#launcher").addEventListener("submit", async e => {
  e.preventDefault();
  const repo_path = repoEl.value.trim();
  const model = chosenModel();
  const claude_version = $("#claude-version").value || null;
  const cpus = Number($("#vm-cpus").value) || null;
  const memory_mb = Number($("#vm-mem").value) || null;
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
});

// ---------- terminal attached to a VM ----------
const THEME = {
  background: "#0b0c0e", foreground: "#e4e5e9", cursor: "#7480e6", cursorAccent: "#0b0c0e", selectionBackground: "#2c3160",
  black: "#151821", red: "#ff6b6b", green: "#4fd18b", yellow: "#ffb547", blue: "#7aa7ff", magenta: "#b9a8ff", cyan: "#5fd7d7", white: "#d5dae3",
  brightBlack: "#5c6577", brightRed: "#ff8a8a", brightGreen: "#74e0a5", brightYellow: "#ffc977", brightBlue: "#9cbcff", brightMagenta: "#cfc2ff", brightCyan: "#86e3e3", brightWhite: "#ffffff",
};
class VmTerminal {
  constructor(id, session, { fontSize = 13, webgl = true, readOnly = false, fixed = null } = {}) {
    this.id = id;
    this.session = session;
    this.fixed = fixed;
    this.el = h("div", { class: fixed ? "term fixed" : "term" });
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
    // Text selected in tmux arrives as OSC 52 (base64): put it on the Mac's clipboard.
    this.xterm.parser.registerOscHandler(52, data => {
      const b64 = data.slice(data.indexOf(";") + 1);
      if (readOnly || !b64 || b64 === "?") return true;
      try {
        const text = new TextDecoder().decode(Uint8Array.from(atob(b64), c => c.charCodeAt(0)));
        navigator.clipboard.writeText(text).then(() => toast("Copied to the clipboard"), () => toast("The browser blocked the clipboard", "err"));
      } catch {}
      return true;
    });
    const enc = new TextEncoder();
    if (!readOnly) this.xterm.onData(d => { if (this.ws?.readyState === 1) this.ws.send(enc.encode(d)); });
    this.xterm.onResize(({ cols, rows }) => { if (this.ws?.readyState === 1) this.ws.send(JSON.stringify({ cols, rows })); });
    this.resizer = new ResizeObserver(() => this.fitSoon());
    this.resizer.observe(fixed ? document.body : this.el);
  }
  fitSoon() {
    if (this.pending) return;
    this.pending = true;
    requestAnimationFrame(() => { this.pending = false; this.fit(); });
  }
  fit() {
    if (!this.el.isConnected || !this.el.offsetWidth) return;
    if (this.fixed) return this.scaleToParent();
    const d = this.fitter.proposeDimensions();
    if (d && (d.cols !== this.xterm.cols || d.rows !== this.xterm.rows)) this.xterm.resize(d.cols, d.rows);
  }
  /** Previews keep the session's real size and are scaled down to fit their cell. */
  scaleToParent() {
    const box = this.el.parentElement;
    const screen = this.el.querySelector(".xterm-screen");
    if (!box || !screen || !screen.offsetWidth) return;
    const k = Math.min(box.clientWidth / screen.offsetWidth, box.clientHeight / screen.offsetHeight, 1);
    this.el.style.transform = `scale(${k})`;
  }
  connect() {
    if (this.ws || this.disposed) return;
    if (this.fixed) this.xterm.resize(this.fixed.cols, this.fixed.rows);
    this.fit();
    if (this.xterm.cols < 20) this.xterm.resize(100, 30);
    const { cols, rows } = this.xterm;
    const view = this.fixed ? "&view=true" : "";
    const ws = new WebSocket(`ws://${location.host}/api/tasks/${this.id}/pty?session=${this.session}&cols=${cols}&rows=${rows}${view}`);
    ws.binaryType = "arraybuffer";
    // Resizes sent while connecting are lost: send the real size once the socket is open.
    ws.onopen = () => { this.fit(); ws.send(JSON.stringify({ cols: this.xterm.cols, rows: this.xterm.rows })); };
    if (this.fixed) {
      // Previews: parse at most 4 times a second, whatever the VM prints (3-5x less browser CPU).
      ws.onmessage = e => {
        (this.chunks ??= []).push(new Uint8Array(e.data));
        this.flushTimer ??= setTimeout(() => {
          this.flushTimer = null;
          const chunks = this.chunks;
          this.chunks = [];
          for (const c of chunks) this.xterm.write(c);
        }, 250);
      };
    } else {
      ws.onmessage = e => this.xterm.write(new Uint8Array(e.data));
    }
    ws.onclose = () => {
      if (this.ws !== ws) return;
      this.ws = null;
      if (!this.disposed && byId(this.id)?.status.state === "running") setTimeout(() => this.connect(), 900);
    };
    this.ws = ws;
  }
  /** Stops streaming (a preview nobody can see); `connect()` resumes and tmux redraws it all. */
  pause() {
    const ws = this.ws;
    this.ws = null;
    ws?.close();
  }

  dispose() {
    this.disposed = true;
    clearTimeout(this.flushTimer);
    this.resizer.disconnect();
    this.ws?.close();
    this.xterm.dispose();
    this.el.remove();
  }
}

// ---------- boot sequence ----------
/** Turns the task's boot log into steps: real events with real timings. */
function bootSteps(t) {
  const host = {}, guest = [];
  for (const line of t.boot_log || []) {
    let m = line.match(/^host: (repository (?:packed|shared)|disk ready) in (\d+) ms/);
    if (m) { host[m[1].startsWith("repository") ? "repository" : m[1]] = `${m[2]} ms`; continue; }
    m = line.match(/^\[([\d.]+)s\] (.*)$/);
    if (m) guest.push({ at: Number(m[1]), text: m[2] });
  }
  const seen = re => guest.find(g => re.test(g.text));
  const secs = g => (g ? `${g.at.toFixed(1)} s` : "");
  const s = t.status.state;
  const steps = [
    { label: "VM slot reserved", done: s !== "queued", detail: s === "queued" ? "waiting for a free slot" : "" },
    { label: "Disk cloned from the image", done: !!host["disk ready"], detail: host["disk ready"] || "" },
    { label: "Debian booted", done: !!seen(/^job start/), detail: secs(seen(/^job start/)) },
    { label: "Repository checked out", done: !!seen(/^repo ready/), detail: secs(seen(/^repo ready/)) },
    { label: "Network up", done: !!seen(/^network ready/), detail: secs(seen(/^network ready/)) },
  ];
  const setup = seen(/^running \.agentvm\/setup\.sh/);
  if (setup) {
    const end = seen(/^setup (done|failed)/);
    steps.push({ label: "Repository setup (.agentvm/setup.sh)", done: !!end, failed: end && /failed/.test(end.text), detail: end ? (/failed/.test(end.text) ? "failed, see setup.log" : secs(end)) : "running" });
  }
  const install = seen(/^installing Claude Code/);
  if (install) steps.push({ label: install.text.replace(/^installing/, "Installing"), done: !!seen(/^Claude Code \d/), detail: secs(seen(/^Claude Code \d/)) });
  const readyEvt = seen(t.interactive ? /^terminal ready/ : /^network ready/);
  steps.push({ label: t.interactive ? "Claude Code ready" : "Agent started", done: t.ready || !!readyEvt, detail: secs(readyEvt) });
  const current = steps.findIndex(x => !x.done);
  return { steps, current: current < 0 ? steps.length : current };
}

const BOOT_MARK = `<svg viewBox="0 0 64 64" class="boot-mark" aria-hidden="true">
  <g class="brackets" stroke-linecap="square" fill="none">
    <path d="M6 20V6h14"/><path d="M44 6h14v14"/><path d="M58 44v14H44"/><path d="M20 58H6V44"/>
  </g>
  <rect class="core" x="22" y="22" width="12" height="20"/>
  <rect class="scan" x="8" y="8" width="48" height="2"/>
</svg>`;

/** Boot screen: built once (the animation never restarts), then `update(t)` every tick. */
function bootPanel(compact = false) {
  const mark = h("div", { class: `boot-mark-wrap${compact ? " small" : ""}` });
  mark.innerHTML = BOOT_MARK;
  const kicker = h("span", { class: "boot-kicker" });
  const now = h("b", { class: "boot-now" });
  const clock = h("span", { class: "boot-clock" });
  const fill = h("i");
  const list = h("ol", { class: "boot-steps" });
  const root = compact
    ? h("div", { class: "boot compact" }, mark, now, h("div", { class: "boot-bar" }, fill), clock)
    : h("div", { class: "boot" }, mark, h("div", { class: "boot-head" }, kicker, now, clock), h("div", { class: "boot-bar" }, fill), list);
  root.update = t => {
    const { steps, current } = bootSteps(t);
    kicker.textContent = t.status.state === "queued" ? "Queued" : "Sealing your machine";
    now.textContent = current < steps.length ? `${steps[current].label}…` : "Opening the terminal…";
    clock.textContent = compact ? `${current} of ${steps.length}, ${age(t)}` : age(t);
    fill.style.width = `${(100 * current) / steps.length}%`;
    if (!compact) {
      list.replaceChildren(...steps.map((x, k) =>
        h("li", { class: x.failed ? "failed" : x.done ? "done" : k === current ? "now" : "" },
          h("span", { class: "tick" }), h("span", { class: "what" }, x.label), h("span", { class: "when" }, x.detail))));
    }
  };
  return root;
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
    this.head = h("header", { class: "view-head" });
    this.root = h("div", { style: "flex:1;min-height:0;display:flex;flex-direction:column" }, this.head, this.grid);
  }
  update() {
    const tasks = state.tasks;
    if (!tasks.length) { this.root.replaceChildren(emptyState()); this.cells.clear(); return; }
    if (!this.grid.isConnected) this.root.replaceChildren(this.head, this.grid);
    const live = tasks.filter(t => !TERMINAL.has(t.status.state)).length;
    const waiting = tasks.filter(t => t.status.state === "running" && t.activity === "waiting").length;
    const finished = tasks.filter(t => TERMINAL.has(t.status.state));
    if (!this.headBuilt) {
      this.headBuilt = true;
      this.sub = h("span", { class: "sub" });
      this.clearBtn = h("button", { class: "btn ghost small", type: "button", onclick: async () => {
        const done = state.tasks.filter(t => TERMINAL.has(t.status.state));
        for (const t of done) await api(`/api/tasks/${t.id}`, { method: "DELETE" });
        toast(`Removed ${plural(done.length, "finished machine")}`);
        loadTasks();
      } });
      this.head.replaceChildren(h("h1", {}, "Machines"), this.sub, h("span", { class: "spacer" }), this.clearBtn);
    }
    const sub = waiting ? `${plural(live, "running machine")}, ${waiting} waiting for you` : `${plural(live, "running machine")}. Click one to work in it.`;
    if (this.sub.textContent !== sub) this.sub.textContent = sub;
    this.clearBtn.hidden = !finished.length;
    const clearText = `Clear ${finished.length} finished`;
    if (this.clearBtn.textContent !== clearText) this.clearBtn.textContent = clearText;
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
    this.mem.replaceChildren(h("b", {}, m ? gb(m.mem_used_mb) : "—"), "RAM", sparkline(t.mem_history, { width: 70, height: 16, color: "var(--ink-3)" }));
    this.portChip ??= h("a", { class: "port-chip", target: "_blank", rel: "noopener", onclick: e => e.stopPropagation() });
    const p0 = t.ports?.find(p => p.kind === "http") ?? t.ports?.[0];
    this.portChip.hidden = !p0;
    if (p0) {
      this.portChip.href = p0.url ?? `http://localhost:${p0.host_port}`;
      this.portChip.title = p0.url ?? `localhost:${p0.host_port}`;
      this.portChip.textContent = `:${p0.port}${t.ports.length > 1 ? ` +${t.ports.length - 1}` : ""}`;
    }
    if (!this.portChip.isConnected) this.proc.before(this.portChip);
    const u = t.usage;
    this.proc.textContent = u?.output_tokens ? `${money(u.cost_usd)}  ${tokens(u.input_tokens + u.output_tokens)} tokens` : m?.top?.[0] ? `${m.top[0].name} ${m.top[0].cpu_pct.toFixed(0)}%` : repoName(t.repo);
    if (!TERMINAL.has(t.status.state) && t.interactive && !(t.status.state === "running" && t.ready)) {
      this.term?.dispose();
      this.term = null;
      this.endedFor = null;
      if (!this.boot || !this.body.contains(this.boot)) { this.boot = bootPanel(true); this.body.replaceChildren(this.boot); }
      this.boot.update(t);
    } else if (t.status.state === "running" && t.interactive) {
      if (!this.term) {
        this.body.replaceChildren();
        this.term = new VmTerminal(t.id, "claude", { fontSize: 12, webgl: false, readOnly: true, fixed: { cols: 160, rows: 48 } });
        this.body.append(this.term.el);
        this.resize ??= new ResizeObserver(() => this.term?.fitSoon());
        this.resize.observe(this.body);
        // Previews stream only while on screen and while the page is visible.
        this.seen ??= new IntersectionObserver(([e]) => { this.visible = e.isIntersecting; this.stream(); });
        this.seen.observe(this.root);
      }
      this.stream();
    } else {
      this.term?.dispose();
      this.term = null;
      const text = TERMINAL.has(t.status.state)
        ? [h("b", {}, label), h("span", {}, repoName(t.repo))]
        : [h("b", {}, label), h("span", {}, t.interactive ? "Debian boots in a few seconds, then Claude opens." : "Running without a terminal.")];
      if (this.endedFor !== label) { this.endedFor = label; this.body.replaceChildren(h("div", { class: "ended" }, ...text)); }
    }
  }
  stream() {
    if (!this.term) return;
    if (this.visible !== false && !document.hidden) this.term.connect();
    else this.term.pause();
  }
  destroy() { this.term?.dispose(); this.resize?.disconnect(); this.seen?.disconnect(); }
}

/** A service of a VM: HTTP ones under the VM's own name, others through a TCP port on the Mac. */
function portLink(p) {
  const name = p.name || "service";
  if (p.kind === "http") {
    const host = p.url.replace(/^http:\/\//, "").replace(/\/?\?.*$/, "");
    return h("a", { class: "port-link", href: p.url, target: "_blank", rel: "noopener",
      title: `${name} on port ${p.port} of this VM.\nEvery VM has its own name, so they can all use port ${p.port}.\nOpens ${p.url}` },
      h("span", { class: "live" }), h("b", {}, name), h("span", { class: "addr" }, host),
      svg("svg", { class: "i ext", viewBox: "0 0 16 16", "aria-hidden": "true" }, svg("path", { d: "M6 3.5H3.5v9h9V10M9 3.5h3.5V7M12.5 3.5 7 9" })));
  }
  const addr = `localhost:${p.host_port}`;
  const copy = h("button", { class: "port-link tcp", type: "button",
    title: `${name} on port ${p.port} of this VM is not a web page: connect to ${addr} (click to copy).` },
    h("span", { class: "live" }), h("b", {}, name), h("span", { class: "addr" }, addr),
    p.host_port !== p.port ? h("span", { class: "moved" }, `${p.port} in the VM`) : null, h("span", { class: "moved" }, "TCP"));
  copy.addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(addr); toast(`Copied ${addr}`); } catch { toast(addr); }
  });
  return copy;
}

/** One machine, full size, with its telemetry. */
class FocusView {
  constructor(id) {
    this.id = id;
    this.session = "claude";
    this.terms = {};
    this.showPanel = localStorage.getItem("agentvm.panel") !== "off";
    this.titleEl = h("span", { class: "title" });
    this.stateEl = h("span", { class: "status" });

    this.segBtns = ["claude", "shell"].map(s => h("button", { type: "button", "aria-pressed": String(s === "claude"), onclick: () => this.show(s) }, s === "claude" ? "Claude" : "Root shell"));
    this.saveBtn = h("button", { class: "btn", type: "button", onclick: () => this.save() }, "Save to repo");
    this.snapBtn = h("button", { class: "btn", type: "button", title: "Save a copy of this whole VM that you can restore later", onclick: () => this.snapshot() }, "Snapshot");
    this.closeBtn = h("button", { class: "btn", type: "button", onclick: () => this.close() }, "Close VM");
    this.autoSel = h("select", { class: "auto-snap", title: "Automatic snapshots of this machine", onchange: e => this.setAuto(e.target.value) });
    this.stopBtn = h("button", { class: "btn ghost danger", type: "button", title: "Power off now without saving", onclick: () => this.stop() }, "Force stop");
    this.panelBtn = h("button", { class: "btn ghost", type: "button", onclick: () => this.togglePanel() }, "Telemetry");
    this.seg = h("div", { class: "seg" }, this.segBtns);
    this.toolbar = h("header", { class: "toolbar" }, h("a", { class: "crumb", href: "#/wall" }, "Machines"), h("span", { class: "crumb-sep" }, "›"),
      h("span", { class: "dot" }), this.titleEl, this.seg, h("div", { class: "snap-group" }, this.snapBtn, this.autoSel), this.saveBtn, this.closeBtn, this.stopBtn, this.panelBtn);
    this.overlay = h("div", { class: "overlay" });
    this.portsBar = h("div", { class: "ports-bar", "aria-label": "Ports open on this Mac" });
    this.dropHint = h("div", { class: "drop-hint" }, h("b", {}, "Drop to copy into the VM"), h("span", {}, "The files go to /mnt/job/uploads and their paths are typed in the terminal."));
    this.screen = h("div", { class: "screen" }, this.overlay, this.dropHint);
    this.screen.addEventListener("dragover", e => {
      if (![...e.dataTransfer.types].includes("Files") || byId(this.id)?.status.state !== "running") return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      this.screen.classList.add("dropping");
    });
    this.screen.addEventListener("dragleave", e => { if (!this.screen.contains(e.relatedTarget)) this.screen.classList.remove("dropping"); });
    this.screen.addEventListener("drop", e => { e.preventDefault(); this.screen.classList.remove("dropping"); this.drop([...e.dataTransfer.files]); });
    this.result = h("div", { class: "result" });
    this.result.hidden = true;
    this.stage = h("section", { class: "stage" }, this.toolbar, this.portsBar, this.screen, this.result);
    this.panel = h("aside", { class: "panel", "aria-label": "Telemetry" });
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
    const ended = TERMINAL.has(s);
    for (const b of [this.saveBtn, this.closeBtn, this.snapBtn, this.seg, this.autoSel]) b.hidden = !t.interactive || ended;
    this.paintAuto(t);
    this.saveBtn.disabled = this.closeBtn.disabled = this.snapBtn.disabled = s !== "running";
    this.stopBtn.hidden = ended;
    this.renderPanel(t, label);
    this.renderPorts(ended ? [] : t.ports || []);
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

  /** What the VM serves, reachable from the Mac: always in view, right under the toolbar. */
  renderPorts(ports) {
    this.portsBar.hidden = !ports.length;
    const key = ports.map(p => `${p.port}:${p.url}:${p.host_port}:${p.name}`).join(",");
    if (key === this.portsKey) return;
    this.portsKey = key;
    this.portsBar.replaceChildren(h("span", { class: "ports-label" }, "Open on this Mac"), ...ports.map(portLink));
  }

  renderPanel(t, label) {
    if (!this.showPanel) return;
    const m = TERMINAL.has(t.status.state) ? null : t.metrics;
    const kids = [];
    if (m) {
      kids.push(
        h("div", { class: "stat" }, h("h3", {}, "CPU"), h("div", { class: "big" }, `${m.cpu_pct.toFixed(0)}%`, h("small", {}, `of ${m.cpus} vCPUs`)), sparkline(t.cpu_history, { width: 250, height: 40, fluid: true })),
        h("div", { class: "stat" }, h("h3", {}, "Memory"), h("div", { class: "big" }, gb(m.mem_used_mb), h("small", {}, `of ${gb(m.mem_total_mb)}`)), sparkline(t.mem_history, { width: 250, height: 40, color: "var(--ink-3)", fluid: true })),
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
    const u = t.usage;
    if (u) {
      kids.unshift(h("div", { class: "stat" }, h("h3", {}, "Claude"),
        h("div", { class: "big" }, money(u.cost_usd), h("small", {}, "at API prices")),
        h("dl", { class: "kv" },
          h("dt", {}, "Tokens in"), h("dd", {}, tokens(u.input_tokens)),
          h("dt", {}, "Tokens out"), h("dd", {}, tokens(u.output_tokens)),
          h("dt", {}, "Lines changed"), h("dd", {}, h("span", { class: "plus" }, `+${u.lines_added}`), " ", h("span", { class: "minus" }, `−${u.lines_removed}`)),
          u.context_pct != null && h("dt", {}, "Context used"), u.context_pct != null && h("dd", {}, `${Math.round(u.context_pct)}%`))));
    }
    if (TERMINAL.has(t.status.state)) kids.unshift(h("p", { class: "closed-note" }, "This VM is closed: its disk is gone, the work is on the branch."));
    kids.push(h("dl", { class: "kv" },
      h("dt", {}, "State"), h("dd", {}, label),
      h("dt", {}, "Model"), h("dd", {}, modelLabel(t.model)),
      h("dt", {}, "Claude Code"), h("dd", {}, t.claude_version ?? "image version"),
      h("dt", {}, "Resources"), h("dd", {}, `${t.cpus} vCPUs, ${gb(t.memory_mb)}`),
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

  /** Copies dropped files into the VM, then types their paths, as the Mac's Terminal does. */
  async drop(files) {
    if (!files.length) return;
    const paths = [];
    for (const f of files) {
      toast(`Copying ${f.name} into the VM…`);
      try {
        const res = await fetch(`/api/tasks/${this.id}/upload`, { method: "POST", body: f, headers: { "x-file-name": encodeURIComponent(f.name) } });
        const data = await res.json().catch(() => ({}));
        if (!res.ok) { toast(data.error || `Could not copy ${f.name}`, "err"); continue; }
        paths.push(data.path);
      } catch { toast(`Could not copy ${f.name}`, "err"); }
    }
    if (!paths.length) return;
    const quote = p => (/^[\w@%+=:,./-]+$/.test(p) ? p : `'${p.replace(/'/g, "'\\''")}'`);
    const term = this.ensureTerm(this.session);
    term.xterm.paste(paths.map(quote).join(" ") + " ");
    term.xterm.focus();
    toast(paths.length === 1 ? `In the VM: ${paths[0]}` : `${paths.length} files in /mnt/job/uploads`);
  }

  paintAuto(t) {
    const def = state.settings?.settings.auto_snapshots?.every_min ?? 30;
    const label = m => (m === 0 ? "off" : m < 60 ? `${m} min` : `${m / 60} h`);
    const key = `${def}:${t.auto_snapshot_min}`;
    if (key === this.autoKey || document.activeElement === this.autoSel) return;
    this.autoKey = key;
    this.autoSel.replaceChildren(
      h("option", { value: "", selected: t.auto_snapshot_min == null }, `Auto: ${label(def)}`),
      ...[0, 5, 15, 30, 60, 120].map(m => h("option", { value: String(m), selected: t.auto_snapshot_min === m }, m === 0 ? "Auto: off" : `Auto: every ${label(m)}`)));
  }
  async setAuto(v) {
    const r = await api(`/api/tasks/${this.id}/auto-snapshots`, { method: "PUT", body: { every_min: v === "" ? null : Number(v) } });
    toast(r.ok ? (v === "" ? "Snapshots follow the settings" : v === "0" ? "No automatic snapshots for this machine" : `A snapshot every ${v} minutes`) : r.data?.error || "Could not change it", r.ok ? "ok" : "err");
    this.autoKey = null;
    loadTasks();
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
    if (this.resultFor === t.status.state) return;
    this.resultFor = t.status.state;
    this.result.hidden = false;
    this.result.replaceChildren(outcome(t), h("div", { class: "row-actions" },
      h("button", { class: "btn ghost small", type: "button", onclick: async () => {
        const r = await api(`/api/tasks/${t.id}`, { method: "DELETE" });
        if (r.ok) { toast("Removed from the list"); location.hash = "#/wall"; loadTasks(); } else toast(r.data?.error || "Could not remove", "err");
      } }, "Remove from the list")));
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
      ["snapshots", "Automatic snapshots", this.autoSnapshots()],
      ["image", "VM image", this.image()],
      ["storage", "Storage", this.storage()],
      ["backups", "Backups to S3", this.backups()],
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
    this.modelCards = h("div", { class: "choices", role: "radiogroup", "aria-label": "Default model" },
      MODELS.map(([v, name, desc]) => h("button", { type: "button", role: "radio", "data-value": v, "aria-checked": String(s.model === v), onclick: () => { this.customModel.value = ""; this.set("model", v); } }, h("b", {}, name), h("span", {}, desc))));
    this.customModel = h("input", { placeholder: "Or a model ID, e.g. claude-opus-5-5", spellcheck: "false", value: MODELS.some(m => m[0] === s.model) ? "" : s.model,
      oninput: e => { const v = e.target.value.trim(); if (v) this.set("model", v); } });
    this.timeoutIn = h("input", { type: "number", min: "1", max: "1440", value: String(Math.round(s.timeout_s / 60)), oninput: e => this.set("timeout_s", Number(e.target.value) * 60) });
    this.repoIn = h("input", { value: s.default_repo ?? "", placeholder: "~/code/my-app", spellcheck: "false", oninput: e => this.set("default_repo", e.target.value.trim() || null) });
    this.agentErr = h("p", { class: "field-error", role: "alert" });
    return [
      h("div", { class: "set" }, h("label", {}, "Default model"), this.modelCards, this.customModel, h("p", { class: "hint" }, "Every launch can pick a different one. Aliases always point to the newest model of the family.")),
      h("div", { class: "pair" },
        h("div", { class: "set" }, h("label", {}, "Time limit for automatic tasks"), h("div", { class: "unit" }, this.timeoutIn, h("span", {}, "minutes")), h("p", { class: "hint" }, "Terminals never time out: you close them.")),
        h("div", { class: "set" }, h("label", {}, "Default repository"), this.repoIn, h("p", { class: "hint" }, "Prefilled in New VM."))),
      this.agentErr];
  }

  autoSnapshots() {
    const a = this.draft.auto_snapshots;
    const label = m => (m === 0 ? "Off" : m < 60 ? `${m} min` : `${m / 60} h`);
    this.snapSeg = h("div", { class: "seg wide intervals", role: "radiogroup", "aria-label": "Snapshot every" },
      [0, 5, 15, 30, 60, 120].map(m => h("button", { type: "button", role: "radio", "data-value": String(m), "aria-checked": String(a.every_min === m),
        onclick: () => { this.set("auto_snapshots", { ...this.draft.auto_snapshots, every_min: m }); this.paintSnapshots(); } }, label(m))));
    this.keepIn = h("input", { type: "number", min: "1", max: "50", value: String(a.keep),
      oninput: e => this.set("auto_snapshots", { ...this.draft.auto_snapshots, keep: Number(e.target.value) }) });
    this.closeSwitch = h("button", { class: "switch", type: "button", role: "switch", "aria-checked": String(a.before_close),
      onclick: () => { this.set("auto_snapshots", { ...this.draft.auto_snapshots, before_close: !this.draft.auto_snapshots.before_close }); this.paintSnapshots(); } }, h("i"));
    return [h("p", { class: "lede" }, "Running machines are saved whole (files, installed packages, Claude's conversation) so you can go back to any point. Copies share unchanged blocks, so they take little space."),
      h("div", { class: "set" }, h("label", {}, "Snapshot every"), this.snapSeg, h("p", { class: "hint" }, "Each machine can use its own interval: open it and pick one from the Auto menu next to Snapshot.")),
      h("div", { class: "pair" },
        h("div", { class: "set" }, h("label", {}, "Keep per machine"), h("div", { class: "unit" }, this.keepIn, h("span", {}, "latest automatic snapshots")), h("p", { class: "hint" }, "Older automatic ones are deleted. Snapshots you take yourself are never deleted.")),
        h("div", { class: "status-line" }, this.closeSwitch, h("div", {}, h("b", {}, "Snapshot before closing"), h("p", { class: "hint" }, "The machine as it was when you closed it, in case you need it again."))))];
  }
  paintSnapshots() {
    const a = this.draft.auto_snapshots;
    for (const b of this.snapSeg.children) b.setAttribute("aria-checked", String(Number(b.dataset.value) === a.every_min));
    this.closeSwitch.setAttribute("aria-checked", String(a.before_close));
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
    this.versionSelect = h("select", { onchange: e => this.set("claude_version", e.target.value) }, versionOptions(false, null, this.draft.claude_version));
    loadReleases().then(() => this.versionSelect.replaceChildren(...versionOptions(false, null, this.draft.claude_version)));
    return [h("p", { class: "lede" }, "Every VM starts as an instant copy of this Debian 13 image with Claude Code preinstalled. Rebuild it to update Claude Code and the system packages; running VMs are not affected."),
      this.goldenFacts,
      h("div", { class: "set narrow" }, h("label", {}, "Claude Code version for the image"), this.versionSelect,
        h("p", { class: "hint" }, "Used by the next rebuild. A single launch can still pick another version; auto-update is off inside the VMs.")),
      h("div", { class: "row-actions" }, this.rebuildBtn, this.goldenMsg), this.goldenLog];
  }

  backups() {
    const presets = {
      aws: { label: "AWS S3", endpoint: "https://s3.eu-central-1.amazonaws.com", region: "eu-central-1", path_style: false, hint: "Endpoint s3.<region>.amazonaws.com." },
      r2: { label: "Cloudflare R2", endpoint: "https://<account-id>.r2.cloudflarestorage.com", region: "auto", path_style: true, hint: "Region is always auto. Use an R2 API token's access key and secret." },
      hetzner: { label: "Hetzner", endpoint: "https://fsn1.your-objectstorage.com", region: "fsn1", path_style: false, hint: "Locations: fsn1, nbg1, hel1. Region matches the location." },
      b2: { label: "Backblaze B2", endpoint: "https://s3.eu-central-003.backblazeb2.com", region: "eu-central-003", path_style: false, hint: "Use an application key; the region is in the endpoint." },
      local: { label: "Local (RustFS / MinIO)", endpoint: "http://127.0.0.1:9100", region: "us-east-1", path_style: true, hint: "Start it with scripts/dev-s3.sh up. Access key agentvm, secret agentvm-local-secret." },
    };
    const f = {};
    const input = (name, attrs = {}) => (f[name] = h("input", { name, spellcheck: "false", autocomplete: "off", ...attrs }));
    this.s3Hint = h("p", { class: "hint" });
    this.s3Msg = h("span", { class: "msg" });
    this.s3Pill = h("span", { class: "pill" });
    const presetSeg = h("div", { class: "seg wide presets", role: "radiogroup", "aria-label": "Provider" },
      Object.entries(presets).map(([key, p]) => h("button", { type: "button", role: "radio", "data-value": key, onclick: () => applyPreset(key) }, p.label)));
    const applyPreset = key => {
      const p = presets[key];
      for (const b of presetSeg.children) b.setAttribute("aria-checked", String(b.dataset.value === key));
      f.endpoint.value = p.endpoint;
      f.region.value = p.region;
      f.path_style.checked = p.path_style;
      this.s3Hint.textContent = p.hint;
      if (key === "local") { f.bucket.value ||= "agentvm-backups"; f.access_key.value ||= "agentvm"; }
    };
    const form = h("form", { class: "s3-form", onsubmit: e => this.saveS3(e, f) },
      h("div", { class: "set" }, h("label", {}, "Provider"), presetSeg, this.s3Hint),
      h("div", { class: "grid3" },
        h("div", { class: "set" }, h("label", {}, "Endpoint"), input("endpoint", { placeholder: "https://…" })),
        h("div", { class: "set" }, h("label", {}, "Region"), input("region", { placeholder: "auto" })),
        h("div", { class: "set" }, h("label", {}, "Bucket"), input("bucket", { placeholder: "agentvm-backups" })),
        h("div", { class: "set" }, h("label", {}, "Folder in the bucket"), input("prefix", { placeholder: "agentvm", value: "agentvm" })),
        h("div", { class: "set" }, h("label", {}, "Access key"), input("access_key")),
        h("div", { class: "set" }, h("label", {}, "Secret key"), input("secret", { type: "password", placeholder: "kept in the Keychain" }))),
      h("label", { class: "check" }, (f.path_style = h("input", { type: "checkbox", name: "path_style" })), "Path-style addresses (endpoint/bucket/key)"),
      h("div", { class: "row-actions" }, h("button", { class: "btn primary", type: "submit" }, "Test & save"), this.s3Msg));
    api("/api/settings/s3").then(r => {
      const c = r.ok ? r.data.config : null;
      this.s3Pill.className = `pill ${c ? "ok" : "err"}`;
      this.s3Pill.textContent = c ? "Connected" : "Not set up";
      if (c) { for (const k of ["endpoint", "region", "bucket", "prefix", "access_key"]) f[k].value = c[k]; f.path_style.checked = c.path_style; }
      if (r.ok && r.data.secret_saved) f.secret.placeholder = "saved in the Keychain; leave empty to keep it";
      if (!c) applyPreset("local");
    });
    return [h("div", { class: "status-line" }, this.s3Pill, h("span", { class: "lede" }, "Back up snapshots to any S3-compatible storage and restore them on this or another Mac.")), form];
  }

  async saveS3(e, f) {
    e.preventDefault();
    const config = { endpoint: f.endpoint.value.trim(), region: f.region.value.trim(), bucket: f.bucket.value.trim(), prefix: f.prefix.value.trim(), access_key: f.access_key.value.trim(), path_style: f.path_style.checked };
    this.s3Msg.className = "msg";
    this.s3Msg.textContent = "Testing the connection…";
    const r = await api("/api/settings/s3", { method: "PUT", body: { config, secret: f.secret.value || null } });
    this.s3Msg.className = `msg ${r.ok ? "ok" : "err"}`;
    this.s3Msg.textContent = r.ok ? "Connected: the bucket accepts uploads. Saved." : r.data?.error || "Could not connect.";
    if (r.ok) { f.secret.value = ""; f.secret.placeholder = "saved in the Keychain; leave empty to keep it"; this.s3Pill.className = "pill ok"; this.s3Pill.textContent = "Connected"; }
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
    if (key === "claude_version") this.goldenMsg.textContent = "Rebuild the image to apply the new version.";
    this.paintResources();
    this.saveState.className = "save-state pending";
    this.saveState.textContent = "Saving…";
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => this.save(), 450);
  }
  async save() {
    const r = await api("/api/settings", { method: "PUT", body: this.draft });
    const err = r.ok ? "" : r.data?.error || "Could not save.";
    const resourceError = /VMs|vCPU|memory/i.test(err) && !/model|Claude Code version/i.test(err);
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
    if (st.ok) this.storageFacts.replaceChildren(fact("Job logs", gb(st.data.jobs_mb)), fact("Running VM disks", gb(st.data.vm_disks_mb)), fact("Snapshots", gb(st.data.snapshots_mb)), fact("VM image", gb(st.data.golden_mb)));
  }

  destroy() { clearTimeout(this.saveTimer); }
}

/** Saved copies of whole VMs, restorable into new machines. */
class SnapshotsView {
  constructor() {
    this.root = h("div", { class: "snapshots" });
    this.built = false;
  }
  async update() {
    if (this.built) return;
    this.built = true;
    await this.refresh();
  }
  async refresh() {
    const r = await api("/api/snapshots");
    const list = r.ok ? r.data : [];
    const fileInput = h("input", { type: "file", accept: ".zst,.tar,.gz", hidden: true, onchange: e => this.importFile(e.target.files[0]) });
    this.importMsg = h("span", { class: "msg" });
    const head = h("header", { class: "view-head" }, h("h1", {}, "Snapshots"),
      h("span", { class: "sub" }, "Instant copies of whole VMs: files, installed packages, Claude's conversation."),
      h("span", { class: "spacer" }), this.importMsg,
      h("button", { class: "btn small", type: "button", onclick: () => fileInput.click() }, "Import from file"), fileInput);
    this.remote = h("section", { class: "remote" });
    this.loadRemote();
    if (!list.length) {
      this.root.replaceChildren(h("div", { class: "snap-inner" }, head, h("div", { class: "card empty-card" },
        h("b", {}, "No snapshots yet"), h("p", { class: "hint" }, "Open a running machine and press Snapshot. Restoring starts a new VM exactly from that point, with Claude continuing its conversation.")), this.remote));
      return;
    }
    this.root.replaceChildren(h("div", { class: "snap-inner" }, head, h("div", { class: "snap-list" }, list.map(sn => this.row(sn))), this.remote));
  }
  row(sn) {
    const msg = h("span", { class: "msg" });
    const restore = h("button", { class: "btn", type: "button", onclick: async () => {
      restore.disabled = true;
      const r = await api(`/api/snapshots/${sn.id}/restore`, { method: "POST" });
      if (r.ok) { await loadTasks(); location.hash = `#/vm/${r.data.id}`; }
      else { msg.className = "msg err"; msg.textContent = r.data?.error || "Restore failed"; restore.disabled = false; }
    } }, "Restore");
    const del = h("button", { class: "btn ghost danger", type: "button", onclick: async () => {
      if (del.dataset.confirm !== "1") { del.dataset.confirm = "1"; del.textContent = "Click again to delete"; setTimeout(() => { del.dataset.confirm = ""; del.textContent = "Delete"; }, 3000); return; }
      const r = await api(`/api/snapshots/${sn.id}`, { method: "DELETE" });
      if (r.ok) this.refresh(); else { msg.className = "msg err"; msg.textContent = r.data?.error || "Delete failed"; }
    } }, "Delete");
    const download = h("a", { class: "btn ghost", href: `/api/snapshots/${sn.id}/export`, download: `${sn.id}.tar.zst`, title: "Download a .tar.zst you can import elsewhere" }, "Download");
    const backup = h("button", { class: "btn ghost", type: "button", onclick: async () => {
      backup.disabled = true; msg.className = "msg"; msg.textContent = "Uploading to S3…";
      const r = await api(`/api/snapshots/${sn.id}/backup`, { method: "POST" });
      msg.className = `msg ${r.ok ? "ok" : "err"}`;
      msg.textContent = r.ok ? `Backed up (${gb(r.data.archive_mb)} compressed)` : r.data?.error || "Backup failed";
      backup.disabled = false;
      if (r.ok) this.loadRemote();
    } }, "Back up to S3");
    return h("article", { class: "snap" },
      h("div", { class: "snap-main" },
        h("b", { class: "snap-name" }, sn.name),
        h("div", { class: "snap-meta" },
          h("span", {}, repoName(sn.repo)),
          h("span", {}, new Date(sn.created_at * 1000).toLocaleString()),
          h("span", {}, `disk image ${gb(sn.size_mb)}`),
          sn.cpus ? h("span", {}, `${sn.cpus} vCPUs, ${gb(sn.memory_mb)}`) : null,
          h("span", {}, modelLabel(sn.model)),
          sn.auto ? h("span", { class: "tag" }, "Automatic") : null)),
      msg, download, backup, del, restore);
  }
  async importFile(file) {
    if (!file) return;
    this.importMsg.className = "msg";
    this.importMsg.textContent = `Importing ${file.name}…`;
    try {
      const res = await fetch("/api/snapshots/import", { method: "POST", body: file, headers: { "content-type": "application/octet-stream" } });
      const data = await res.json().catch(() => ({}));
      this.importMsg.className = `msg ${res.ok ? "ok" : "err"}`;
      this.importMsg.textContent = res.ok ? `Imported ${data.name}` : data.error || "Import failed";
      if (res.ok) this.refresh();
    } catch { this.importMsg.className = "msg err"; this.importMsg.textContent = "Import failed"; }
  }
  async loadRemote() {
    const r = await api("/api/backups");
    if (!r.ok) {
      const notSet = /not configured/i.test(r.data?.error || "");
      this.remote.replaceChildren(h("h2", {}, "In S3"), h("p", { class: "hint" }, notSet ? "Set up a bucket in Settings → Backups to S3 to keep copies off this Mac." : r.data?.error || "Could not reach S3."));
      return;
    }
    const rows = r.data.map(b => {
      const msg = h("span", { class: "msg" });
      const restore = h("button", { class: "btn", type: "button", disabled: b.local, title: b.local ? "Already on this Mac" : "", onclick: async () => {
        restore.disabled = true; msg.className = "msg"; msg.textContent = "Downloading…";
        const x = await api(`/api/backups/${b.snapshot.id}/restore`, { method: "POST" });
        msg.className = `msg ${x.ok ? "ok" : "err"}`; msg.textContent = x.ok ? "Now in your snapshots" : x.data?.error || "Download failed";
        if (x.ok) { this.built = false; this.update(); }
      } }, b.local ? "On this Mac" : "Bring to this Mac");
      const del = h("button", { class: "btn ghost danger", type: "button", onclick: async () => {
        if (del.dataset.confirm !== "1") { del.dataset.confirm = "1"; del.textContent = "Click again to delete"; setTimeout(() => { del.dataset.confirm = ""; del.textContent = "Delete from S3"; }, 3000); return; }
        const x = await api(`/api/backups/${b.snapshot.id}`, { method: "DELETE" });
        if (x.ok) this.loadRemote(); else { msg.className = "msg err"; msg.textContent = x.data?.error || "Delete failed"; }
      } }, "Delete from S3");
      return h("article", { class: "snap remote-snap" },
        h("div", { class: "snap-main" }, h("b", { class: "snap-name" }, b.snapshot.name),
          h("div", { class: "snap-meta" }, h("span", {}, repoName(b.snapshot.repo)), h("span", {}, new Date(b.snapshot.created_at * 1000).toLocaleString()), h("span", {}, `${gb(b.archive_mb)} compressed`))),
        msg, del, restore);
    });
    this.remote.replaceChildren(h("h2", {}, "In S3"), rows.length ? h("div", { class: "snap-list" }, rows) : h("p", { class: "hint" }, "No backups in the bucket yet."));
  }
  destroy() {}
}

// ---------- routing ----------
function route() {
  const hash = location.hash || "#/wall";
  for (const a of document.querySelectorAll("[data-nav]")) a.setAttribute("aria-current", hash.startsWith(`#/${a.dataset.nav}`) || (a.dataset.nav === "wall" && hash.startsWith("#/vm/")) ? "page" : "false");
  renderSide();
  const vm = hash.match(/^#\/vm\/(.+)$/);
  if (vm) {
    if (current instanceof FocusView && current.id === vm[1]) return;
    mount(new FocusView(vm[1]));
  } else if (hash.startsWith("#/snapshots")) {
    if (!(current instanceof SnapshotsView)) mount(new SnapshotsView());
  } else if (hash.startsWith("#/settings")) {
    if (!(current instanceof SettingsView)) mount(new SettingsView());
  } else if (!(current instanceof WallView)) {
    mount(new WallView());
  }
}
window.addEventListener("hashchange", route);
// A hidden tab streams nothing to its previews.
document.addEventListener("visibilitychange", () => { if (current instanceof WallView) for (const c of current.cells.values()) c.stream(); });

(async () => {
  await Promise.all([loadSettings(), loadStatus()]);
  await loadTasks();
  repoEl.value = recentRepos()[0] ?? "";
  updateLaunch();
  route();
  setInterval(loadTasks, 1000);
  setInterval(loadStatus, 3000);
})();
