// One entry of Claude's conversation: a prompt, a message, a tool call, or its folded result.
import { memo } from "react";
import type { HistoryEntry } from "../../../api/generated/HistoryEntry";

const time = (at: string) => {
  const d = new Date(at);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
};
const lines = (text: string) => {
  const n = text ? text.split("\n").length : 0;
  return n === 1 ? "1 line" : `${n} lines`;
};

// Memoized: entries never change once read; only new ones are drawn.
export const Entry = memo(function Entry({ e }: { e: HistoryEntry }) {
  const meta = (
    <span className="at">
      {time(e.at)}
      {e.sidechain ? " · subagent" : ""}
    </span>
  );
  switch (e.kind) {
    case "user":
      return (
        <div className="turn user">
          {meta}
          <p>{e.text}</p>
        </div>
      );
    case "assistant":
      return (
        <div className="turn claude">
          {meta}
          <p>{e.text}</p>
        </div>
      );
    case "tool_use":
      return (
        <div className="turn tool">
          {meta}
          <code>
            <b>{e.name}</b> {e.input}
          </code>
        </div>
      );
    case "tool_result":
      return (
        <details className={`turn tool-out${e.is_error ? " error" : ""}`}>
          <summary>
            {e.is_error ? "Error" : "Result"} <span className="size">· {lines(e.output)}</span>
          </summary>
          <pre>{e.output}</pre>
        </details>
      );
  }
});
