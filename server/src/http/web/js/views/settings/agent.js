/* Settings › Agent: default model, time limit and default repository. */
import { h } from "../../dom.js";
import { isKnownModel, MODELS } from "../../models.js";

export function agentSection({ draft: s, set }) {
  const modelCards = h("div", { class: "choices", role: "radiogroup", "aria-label": "Default model" },
    MODELS.map(([v, name, desc]) => h("button", { type: "button", role: "radio", "data-value": v, "aria-checked": String(s.model === v), onclick: () => { customModel.value = ""; set("model", v); } }, h("b", {}, name), h("span", {}, desc))));
  const customModel = h("input", { placeholder: "Or a model ID, e.g. claude-opus-5-5", spellcheck: "false", value: isKnownModel(s.model) ? "" : s.model,
    oninput: e => { const v = e.target.value.trim(); if (v) set("model", v); } });
  const timeoutIn = h("input", { type: "number", min: "1", max: "1440", value: String(Math.round(s.timeout_s / 60)), oninput: e => set("timeout_s", Number(e.target.value) * 60) });
  const repoIn = h("input", { value: s.default_repo ?? "", placeholder: "~/code/my-app", spellcheck: "false", oninput: e => set("default_repo", e.target.value.trim() || null) });
  const error = h("p", { class: "field-error", role: "alert" });
  return {
    body: [
      h("div", { class: "set" }, h("label", {}, "Default model"), modelCards, customModel, h("p", { class: "hint" }, "Every launch can pick a different one. Aliases always point to the newest model of the family.")),
      h("div", { class: "pair" },
        h("div", { class: "set" }, h("label", {}, "Time limit for automatic tasks"), h("div", { class: "unit" }, timeoutIn, h("span", {}, "minutes")), h("p", { class: "hint" }, "Terminals never time out: you close them.")),
        h("div", { class: "set" }, h("label", {}, "Default repository"), repoIn, h("p", { class: "hint" }, "Prefilled in New VM."))),
      error],
    changed(key, value) {
      if (key === "model") for (const b of modelCards.children) b.setAttribute("aria-checked", String(b.dataset.value === value));
    },
    showError: text => (error.textContent = text),
  };
}
