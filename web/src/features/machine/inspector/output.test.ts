import { expect, it } from "vitest";
import { outputText, appendOutput } from "./output";
it("renders normalized agent text, tools and errors without invented usage", () => {
  expect(outputText({ kind: "text", text: "Changed the file" })).toBe("Changed the file");
  expect(outputText({ kind: "tool_use", name: "shell", summary: "bun test" })).toBe("shell: bun test");
  expect(outputText({ kind: "tool_result", is_error: true, summary: "Failed" })).toBe("Error: Failed");
  expect(outputText({ kind: "unparsed", raw: "\u001b[31mReadable\u001b[0m" })).toBe("Readable");
});
it("keeps cursor-paged output unique and bounds the rendered history", () => {
  const item = (seq: number) => ({
    seq,
    item: { type: "agent" as const, data: { kind: "text" as const, text: `${seq}` } },
  });
  expect(appendOutput([item(1)], [item(1), item(2)]).map((i) => i.seq)).toEqual([1, 2]);
  expect(
    appendOutput(
      Array.from({ length: 500 }, (_, n) => item(n)),
      [item(500)],
    ).at(0)?.seq,
  ).toBe(1);
});
