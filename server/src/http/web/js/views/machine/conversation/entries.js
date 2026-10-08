/* One entry of Claude's conversation: a prompt, a message, a tool call or its (folded) result. */
import { h } from "../../../dom.js";

const time = at => { const d = new Date(at); return isNaN(d) ? "" : d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }); };

const lines = text => { const n = text ? text.split("\n").length : 0; return n === 1 ? "1 line" : `${n} lines`; };

export function entry(e) {
  const meta = h("span", { class: "at" }, time(e.at), e.sidechain && " · subagent");
  switch (e.kind) {
    case "user": return h("div", { class: "turn user" }, meta, h("p", {}, e.text));
    case "assistant": return h("div", { class: "turn claude" }, meta, h("p", {}, e.text));
    case "tool_use": return h("div", { class: "turn tool" }, meta, h("code", {}, h("b", {}, e.name), " ", e.input));
    case "tool_result": return h("details", { class: `turn tool-out${e.is_error ? " error" : ""}` },
      h("summary", {}, e.is_error ? "Error" : "Result", h("span", { class: "size" }, ` · ${lines(e.output)}`)), h("pre", {}, e.output));
    default: return null;
  }
}

/** Tokens per model call, counted once per message (Claude Code repeats them on each block). */
export function turns(entries) {
  const seen = new Set(), out = [];
  for (const e of entries) {
    if (!e.usage || !e.message_id || seen.has(e.message_id)) continue;
    seen.add(e.message_id);
    out.push({ at: Date.parse(e.at) / 1000, ...e.usage });
  }
  return out.filter(t => !isNaN(t.at));
}
