import type { OutputItem } from "../../../api/generated/OutputItem";
export type { OutputItem } from "../../../api/generated/OutputItem";
export type { OutputPage } from "../../../api/generated/OutputPage";
import { readableReason } from "../outcome/explain";

export type OutputEvent = {
  kind: string;
  text?: string;
  name?: string;
  summary?: string;
  raw?: string;
  model?: string;
  attempt?: number;
  is_error?: boolean;
};
export function outputText(e: OutputEvent): string {
  const text =
    e.kind === "tool_use"
      ? `${e.name ?? "Tool"}: ${e.summary ?? ""}`
      : e.kind === "tool_result"
        ? `${e.is_error ? "Error: " : "Result: "}${e.summary ?? ""}`
        : e.kind === "retry"
          ? `Retry ${e.attempt ?? ""}`
          : e.kind === "init"
            ? `Agent started${e.model ? ` · ${e.model}` : ""}`
            : e.kind === "unparsed"
              ? (e.raw ?? "")
              : `${e.is_error ? "Error: " : ""}${e.text ?? e.summary ?? ""}`;
  return readableReason(text);
}
export function appendOutput(current: OutputItem[], incoming: OutputItem[]): OutputItem[] {
  return [...new Map([...current, ...incoming].map((i) => [i.seq, i])).values()].slice(-500);
}
