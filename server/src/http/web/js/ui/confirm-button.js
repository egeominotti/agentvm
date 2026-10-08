import { h } from "../dom.js";

/** A delete button that acts on a second click within 3 seconds. */
export function confirmButton(label, onConfirm) {
  const btn = h("button", { class: "btn ghost danger", type: "button", onclick: () => {
    if (btn.dataset.confirm !== "1") {
      btn.dataset.confirm = "1";
      btn.textContent = "Click again to delete";
      setTimeout(() => { btn.dataset.confirm = ""; btn.textContent = label; }, 3000);
      return;
    }
    onConfirm();
  } }, label);
  return btn;
}
