// The changes a finished machine brought to its branch: a summary, then a block per file.

import { useQuery } from "@tanstack/react-query";
import { memo } from "react";
import { plural } from "../../../lib/format";
import { parseDiff } from "./diff";

// Memoized: the machine view re-renders every second; a long diff has nothing new to draw.
export const DiffView = memo(function DiffView({ id }: { id: string }) {
  const diff = useQuery({
    queryKey: ["diff", id],
    queryFn: async () => {
      const r = await fetch(`/api/tasks/${id}/diff`);
      if (!r.ok) throw new Error(`the server answered ${r.status}`);
      return parseDiff(await r.text());
    },
    // A finished branch never changes: read once, unless the read failed.
    staleTime: Number.POSITIVE_INFINITY,
  });
  if (diff.error) {
    return (
      <p className="msg err">
        Could not read the changes: {diff.error.message}.{" "}
        <button type="button" className="link" onClick={() => diff.refetch()}>
          Try again
        </button>
      </p>
    );
  }
  const files = diff.data ?? [];
  if (!files.length) return null;
  const add = files.reduce((n, f) => n + f.add, 0);
  const del = files.reduce((n, f) => n + f.del, 0);
  return (
    <section className="diff" aria-label="Changes">
      <p className="diff-summary">
        {plural(files.length, "file")} changed, <span className="plus">+{add}</span>{" "}
        <span className="minus">−{del}</span>
      </p>
      {files.map((f, i) => (
        <details key={f.path} className="diff-file" open={files.length <= 4 || i === 0}>
          <summary>
            <span className="path" title={f.path}>
              {f.path}
            </span>
            <span className="plus">+{f.add}</span>
            <span className="minus">−{f.del}</span>
          </summary>
          <div className="diff-body">
            {f.lines.map((l, k) => (
              <div
                // Lines of a diff can repeat: their place is their identity.
                // biome-ignore lint/suspicious/noArrayIndexKey: the list never reorders
                key={k}
                className={l.startsWith("@@") ? "h" : l.startsWith("+") ? "a" : l.startsWith("-") ? "d" : ""}
              >
                {l}
              </div>
            ))}
          </div>
        </details>
      ))}
    </section>
  );
});
