import { expect, it } from "vitest";
import { memoryChoices } from "./memory";

it("offers what the server accepts, even on a Mac with little memory", () => {
  expect(memoryChoices(64 * 1024, 4096)).toEqual([1024, 2048, 4096, 6144, 8192, 12288, 16384]);
  // 8 GB Mac: the server still allows 1 GB per VM.
  expect(memoryChoices(8 * 1024, 1024)).toEqual([1024]);
});

it("always offers the value in use, even when it is not a usual size", () => {
  expect(memoryChoices(64 * 1024, 3000)).toContain(3000);
  expect(memoryChoices(64 * 1024, 3000)).toEqual([...memoryChoices(64 * 1024, 3000)].sort((a, b) => a - b));
});
