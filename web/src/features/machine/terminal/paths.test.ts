import { expect, it } from "vitest";
import { shellQuote } from "./paths";

it("quotes paths only when the shell needs it", () => {
  expect(shellQuote("/mnt/job/uploads/a.png")).toBe("/mnt/job/uploads/a.png");
  expect(shellQuote("/mnt/job/uploads/my file.png")).toBe("'/mnt/job/uploads/my file.png'");
  expect(shellQuote("/mnt/job/uploads/it's.txt")).toBe("'/mnt/job/uploads/it'\\''s.txt'");
});
