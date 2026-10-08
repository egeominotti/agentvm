/* Settings › Claude account: the subscription token kept in the Keychain. */
import { api } from "../../api.js";
import { h } from "../../dom.js";
import { loadStatus, state } from "../../state.js";

export function accountSection() {
  const pill = h("span", { class: "pill" });
  const msg = h("span", { class: "msg" });
  const input = h("input", { name: "token", type: "password", placeholder: "Paste a token: sk-ant-oat01-…", autocomplete: "off", spellcheck: "false" });

  /** Whether the server has a token. */
  function paint() {
    const ok = state.status?.token;
    pill.className = `pill ${ok ? "ok" : "err"}`;
    pill.textContent = ok ? "Connected" : "Missing";
  }

  async function saveToken(e) {
    e.preventDefault();
    const r = await api("/api/settings/token", { method: "PUT", body: { token: input.value } });
    msg.className = `msg ${r.ok ? "ok" : "err"}`;
    msg.textContent = r.ok ? "Saved in the Keychain. New VMs use it." : r.data?.error || "Could not save the token.";
    if (r.ok) { input.value = ""; await loadStatus(); paint(); }
  }

  const form = h("form", { class: "inline-form", onsubmit: saveToken }, input, h("button", { class: "btn", type: "submit" }, "Save token"));
  return {
    body: [
      h("div", { class: "status-line" }, pill, h("span", { class: "lede" }, "A long-lived token of your Claude subscription, stored in the macOS Keychain.")),
      form, msg,
      h("p", { class: "hint" }, "Create one in a terminal with ", h("code", {}, "claude setup-token"), ". Running VMs keep the token they started with.")],
    paint,
  };
}
