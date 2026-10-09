import { expect, test } from "vitest";
import { panelShown } from "./panel";

test("a wide window shows the details as docked, as last chosen", () => {
  expect(panelShown({ narrow: false, docked: true, drawer: false })).toBe(true);
  expect(panelShown({ narrow: false, docked: false, drawer: true })).toBe(false);
});

test("a narrow window never covers the terminal unless the details are asked for", () => {
  expect(panelShown({ narrow: true, docked: true, drawer: false })).toBe(false);
  expect(panelShown({ narrow: true, docked: false, drawer: true })).toBe(true);
});
