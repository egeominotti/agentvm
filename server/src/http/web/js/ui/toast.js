import { $, h } from "../dom.js";

/** Short message in the corner: never shifts the layout. `kind` is "ok" or "err". */
export function toast(text, kind = "ok") {
  let box = $("#toasts");
  if (!box) { box = h("div", { id: "toasts", class: "toasts", role: "status", "aria-live": "polite" }); document.body.append(box); }
  const item = h("div", { class: `toast ${kind}` }, text);
  box.append(item);
  setTimeout(() => { item.classList.add("out"); setTimeout(() => item.remove(), 300); }, kind === "err" ? 6000 : 3500);
}
