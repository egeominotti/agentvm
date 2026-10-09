// How a machine ended and what to do with its work: the branch and the commands to use it.
import { useState } from "react";
import type { TaskDto } from "../../../api/generated/TaskDto";
import { Button } from "../../../components/Button";
import { plural, shortPath } from "../../../lib/format";
import { statusOf } from "../../../lib/task";

/** [title, explanation] for a failure reason reported by the server. */
function explain(reason: string): [string, string] {
  if (reason === "timeout") return ["Time limit reached", "The agent was stopped after the time limit in Settings."];
  if (reason === "guest_no_result") return ["The VM shut down without a result", "Diagnostics show its last logs."];
  if (reason.startsWith("claude_exit")) return ["Claude stopped with an error", "Diagnostics show what it printed."];
  if (reason.startsWith("vm_error")) return ["The VM did not start", "Virtualization.framework reported an error."];
  if (reason.startsWith("fetch_failed"))
    return ["Could not import the branch", "The work finished but git fetch failed."];
  return ["Failed", "Diagnostics show what happened."];
}

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
          <code className="reason">{s.reason}</code>
        </>
      ) : null}
    </section>
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
