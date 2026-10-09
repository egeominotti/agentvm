import { expect, it } from "vitest";
import { DeviceAttributes } from "./da-replies";

it("turns restty's answer to a secondary query (ESC [ > c) into the secondary answer", () => {
  const da = new DeviceAttributes();
  da.output("\x1b[c\x1b[>c\x1b[>q");
  expect(da.reply("\x1b[?1;2c")).toBe("\x1b[?1;2c");
  expect(da.reply("\x1b[?1;2c")).toBe("\x1b[>1;10;0c");
});

it("follows the queries across chunks and leaves other replies alone", () => {
  const da = new DeviceAttributes();
  da.output("prompt \x1b[");
  da.output(">0c and more");
  expect(da.reply("\x1b]11;rgb:1e1e/1e1e/2e2e\x07")).toBe("\x1b]11;rgb:1e1e/1e1e/2e2e\x07");
  expect(da.reply("\x1b[?1;2c")).toBe("\x1b[>1;10;0c");
  expect(da.reply("typed keys")).toBe("typed keys");
});

it("a primary query keeps the primary answer", () => {
  const da = new DeviceAttributes();
  da.output("\x1b[0c");
  expect(da.reply("\x1b[?1;2c")).toBe("\x1b[?1;2c");
  expect(da.reply("\x1b[?1;2c")).toBe("\x1b[?1;2c");
});
