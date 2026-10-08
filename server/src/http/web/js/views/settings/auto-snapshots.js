/* Settings › Automatic snapshots: how often running machines are saved whole, and how many copies to keep. */
import { h } from "../../dom.js";

const label = m => (m === 0 ? "Off" : m < 60 ? `${m} min` : `${m / 60} h`);

export function autoSnapshotsSection({ draft, set }) {
  const a = draft.auto_snapshots;
  const change = patch => set("auto_snapshots", { ...draft.auto_snapshots, ...patch });
  const interval = h("div", { class: "seg wide intervals", role: "radiogroup", "aria-label": "Snapshot every" },
    [0, 5, 15, 30, 60, 120].map(m => h("button", { type: "button", role: "radio", "data-value": String(m), "aria-checked": String(a.every_min === m),
      onclick: () => { change({ every_min: m }); paint(); } }, label(m))));
  const keepIn = h("input", { type: "number", min: "1", max: "50", value: String(a.keep),
    oninput: e => change({ keep: Number(e.target.value) }) });
  const closeSwitch = h("button", { class: "switch", type: "button", role: "switch", "aria-checked": String(a.before_close),
    onclick: () => { change({ before_close: !draft.auto_snapshots.before_close }); paint(); } }, h("i"));

  function paint() {
    const now = draft.auto_snapshots;
    for (const b of interval.children) b.setAttribute("aria-checked", String(Number(b.dataset.value) === now.every_min));
    closeSwitch.setAttribute("aria-checked", String(now.before_close));
  }

  return {
    body: [h("p", { class: "lede" }, "Running machines are saved whole (files, installed packages, Claude's conversation) so you can go back to any point. Copies share unchanged blocks, so they take little space."),
      h("div", { class: "set" }, h("label", {}, "Snapshot every"), interval, h("p", { class: "hint" }, "Each machine can use its own interval: open it and pick one from the Auto menu next to Snapshot.")),
      h("div", { class: "pair" },
        h("div", { class: "set" }, h("label", {}, "Keep per machine"), h("div", { class: "unit" }, keepIn, h("span", {}, "latest automatic snapshots")), h("p", { class: "hint" }, "Older automatic ones are deleted. Snapshots you take yourself are never deleted.")),
        h("div", { class: "status-line" }, closeSwitch, h("div", {}, h("b", {}, "Snapshot before closing"), h("p", { class: "hint" }, "The machine as it was when you closed it, in case you need it again."))))],
  };
}
