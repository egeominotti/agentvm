import { expect, it } from "vitest";
import type { HistoryEntry } from "../../../api/generated/HistoryEntry";
import { turns } from "./turns";

const usage = (output_tokens: number) => ({
  input_tokens: 10,
  output_tokens,
  cache_read_input_tokens: 0,
  cache_creation_input_tokens: 0,
});

it("counts each model call once, even when Claude Code repeats it on every block", () => {
  const entries: HistoryEntry[] = [
    { at: "2026-10-09T00:26:25Z", sidechain: false, message_id: "m1", usage: usage(30), kind: "assistant", text: "a" },
    {
      at: "2026-10-09T00:26:25Z",
      sidechain: false,
      message_id: "m1",
      usage: usage(30),
      kind: "tool_use",
      name: "Edit",
      input: "{}",
    },
    { at: "2026-10-09T00:26:30Z", sidechain: false, message_id: "m2", usage: usage(19), kind: "assistant", text: "b" },
    { at: "2026-10-09T00:26:31Z", sidechain: false, message_id: null, usage: null, kind: "user", text: "c" },
  ];
  expect(turns(entries).map((t) => t.output_tokens)).toEqual([30, 19]);
  expect(turns(entries)[0]?.at).toBe(Date.parse("2026-10-09T00:26:25Z") / 1000);
});
