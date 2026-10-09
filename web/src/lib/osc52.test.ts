import { expect, it } from "vitest";
import { Osc52Scanner } from "./osc52";

const b64 = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));

it("finds a clipboard write ended by BEL or by ST", () => {
  const s = new Osc52Scanner();
  expect(s.push(`before\x1b]52;c;${b64("hello")}\x07after`)).toEqual(["hello"]);
  expect(s.push(`\x1b]52;c;${b64("ünïcode ✓")}\x1b\\`)).toEqual(["ünïcode ✓"]);
});

it("finds one split across chunks, as a socket delivers it", () => {
  const s = new Osc52Scanner();
  const whole = `x\x1b]52;c;${b64("split text")}\x07y`;
  expect(s.push(whole.slice(0, 9))).toEqual([]);
  expect(s.push(whole.slice(9, 15))).toEqual([]);
  expect(s.push(whole.slice(15))).toEqual(["split text"]);
});

it("ignores a clipboard query and anything that is not base64", () => {
  const s = new Osc52Scanner();
  expect(s.push("\x1b]52;c;?\x07")).toEqual([]);
  expect(s.push("\x1b]52;c;!!not base64!!\x07")).toEqual([]);
  expect(s.push("plain output without any sequence")).toEqual([]);
});
