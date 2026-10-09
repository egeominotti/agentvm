// How a machine ended and what to do with its work: the branch and the commands to use it.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../../api/client";
import type { SnapshotMeta } from "../../../api/generated/SnapshotMeta";
import type { TaskDto } from "../../../api/generated/TaskDto";
import { keys } from "../../../api/queries";
import { go } from "../../../app/router";
import { Button } from "../../../components/Button";
import { plural, shortPath } from "../../../lib/format";
import { statusOf } from "../../../lib/task";
import { explain, readableReason } from "./explain";

export function Outcome({ task: t }: { task: TaskDto }) {
  const s = t.status;
  const repo = shortPath(t.repo).replace(/ /g, "\\ ");
  return (
    <section className={`outcome tone-${statusOf(t).tone}`}>
      {s.state === "done" ? (
        <>
          <h2>
            {plural(s.commits, "commit")} on {s.branch}
          </h2>
          <p>Already in your repository. To try the work, or to merge it:</p>
          <Command text={`git -C ${repo} switch ${s.branch}`} />
          <Command text={`git -C ${repo} merge ${s.branch}`} />
        </>
      ) : s.state === "no_changes" ? (
        <>
          <h2>Closed without changes</h2>
          <p>Nothing was committed, so no branch was created.</p>
        </>
      ) : s.state === "stopped" ? (
        <>
          <h2>Force stopped</h2>
          <p>Anything saved before is still on {t.branch}.</p>
        </>
      ) : s.state === "failed" ? (
        <>
          <h2>{explain(s.reason)[0]}</h2>
          <p>{explain(s.reason)[1]}</p>
          <details className="reason">
            <summary>What the server reported</summary>
            <pre>{readableReason(s.reason)}</pre>
          </details>
        </>
      ) : null}
      {s.state === "failed" || s.state === "stopped" ? <ResumeKeptDisk task={t} /> : null}
    </section>
  );
}

/** A machine that ended before its work reached the repository has its disk kept as a snapshot:
 *  one click starts a new VM from it, Claude's conversation included. */
function ResumeKeptDisk({ task: t }: { task: TaskDto }) {
  const qc = useQueryClient();
  const snapshots = useQuery({ queryKey: ["snapshots"], queryFn: () => api<SnapshotMeta[]>("/api/snapshots") });
  const kept = snapshots.data?.find((s) => s.source_task === t.id && s.name.startsWith("Interrupted: "));
  const resume = useMutation({
    mutationFn: (id: string) => api<{ id: string }>(`/api/snapshots/${id}/restore`, "POST"),
    onSuccess: async (r) => {
      await qc.invalidateQueries({ queryKey: keys.tasks });
      go(`#/vm/${r.id}`);
    },
  });
  if (!kept) return null;
  return (
    <div className="resume">
      <p>Its disk was kept, with everything it had not saved yet.</p>
      <Button variant="primary" disabled={resume.isPending} onClick={() => resume.mutate(kept.id)}>
        {resume.isPending ? "Starting…" : "Resume in a new VM"}
      </Button>
      {resume.error ? <span className="msg err">{resume.error.message}</span> : null}
    </div>
  );
}

function Command({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="cmd">
      <code>{text}</code>
      <Button
        size="sm"
        onClick={() =>
          navigator.clipboard.writeText(text).then(() => {
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          })
        }
      >
        {copied ? "Copied" : "Copy"}
      </Button>
    </div>
  );
}
