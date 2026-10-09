// The changes a finished machine brought to its branch: a summary, then a block per file.
import { useQuery } from "@tanstack/react-query";
import { plural } from "../../../lib/format";
import { parseDiff } from "./diff";

export function DiffView({ id }: { id: string }) {
  const diff = useQuery({
    queryKey: ["diff", id],
    queryFn: async () => {
      const r = await fetch(`/api/tasks/${id}/diff`);
      return r.ok ? parseDiff(await r.text()) : [];
    },
    staleTime: Number.POSITIVE_INFINITY,
  });
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
}
