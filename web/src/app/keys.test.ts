import { expect, it } from "vitest";
import { isTyping } from "./keys";

it("a key typed into a field or a terminal is never a shortcut", () => {
  const input = document.createElement("input");
  const area = document.createElement("textarea");
  const term = document.createElement("div");
  term.className = "xterm";
  const inTerm = document.createElement("textarea");
  term.append(inTerm);
  const editable = document.createElement("div");
  editable.contentEditable = "true";
  for (const el of [input, area, inTerm, editable]) expect(isTyping(el)).toBe(true);
  expect(isTyping(document.createElement("button"))).toBe(false);
  expect(isTyping(document.body)).toBe(false);
  expect(isTyping(null)).toBe(false);
});
