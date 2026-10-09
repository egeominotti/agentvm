// Every machine: a summary, then the running ones (live cards or a table) and the finished ones.
import { useState } from "react";
import { useStatus, useTasks } from "../../api/queries";
import { Button } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { gb, money, tokens } from "../../lib/format";
import { isEnded, isQueued } from "../../lib/task";
import { openLauncher } from "../launcher/open";
import { EmptyState } from "./EmptyState";
import { liveKind } from "./filter";
import { FinishedTable } from "./FinishedTable";
import { RunningView } from "./RunningView";

type Tab = "running" | "finished";

export function Wall() {
  const q = useTasks();
  const host = useStatus().data;
  const [tab, setTab] = useState<Tab>("running");
  const tasks = q.data ?? [];
  const live = tasks.filter((t) => !isEnded(t));
  const ended = tasks.filter(isEnded);

  if (q.error && !q.data) {
    return (
      <div className="empty-state">
        <b>Cannot reach agentvm</b>
        <span>{q.error.message} Retrying…</span>
      </div>
    );
  }
  if (q.isSuccess && !tasks.length) return <EmptyState />;

  const queued = live.filter(isQueued).length;
  const waiting = live.filter((t) => liveKind(t) === "waiting").length;
  const working = live.filter((t) => liveKind(t) === "working").length;
  const memUsed = live.reduce((a, t) => a + (t.metrics?.mem_used_mb ?? 0), 0);
  const spend = tasks.reduce((a, t) => a + (t.usage?.cost_usd ?? 0), 0);
  const used = tasks.reduce((a, t) => a + (t.usage ? t.usage.input_tokens + t.usage.output_tokens : 0), 0);
  return (
    <div className="wall-view">
      <header className="view-head">
        <h1>Machines</h1>
        <span className="sub">Each agent in a VM of its own: open one to drive it, or watch them all here.</span>
        <span className="tb-gap" />
        <Button variant="primary" onClick={openLauncher}>
          <Icon name="plus" />
          New VM <kbd>⌘K</kbd>
        </Button>
      </header>
      <div className="page-body">
        <dl className="stat-strip">
          <div>
            <dt>Running</dt>
            <dd>
              {live.length - queued}
              {queued ? <small>{queued} in the queue</small> : null}
            </dd>
          </div>
          <div className={waiting ? "tone-wait" : undefined}>
            <dt>Waiting for you</dt>
            <dd>{waiting}</dd>
          </div>
          <div className={working ? "tone-work" : undefined}>
            <dt>Working</dt>
            <dd>{working}</dd>
          </div>
          <div>
            <dt>Memory in use</dt>
            <dd>
              {gb(memUsed)}
              {host ? <small>of {gb(host.ram_committed_mb)} reserved</small> : null}
            </dd>
          </div>
          <div>
            <dt>Claude</dt>
            <dd>
              {money(spend)}
              {used ? <small>{tokens(used)} tokens</small> : null}
            </dd>
          </div>
        </dl>
        <div className="line-tabs" role="tablist" aria-label="Machines">
          {(
            [
              ["running", "Running", live.length],
              ["finished", "Finished", ended.length],
            ] as const
          ).map(([id, label, count]) => (
            <button key={id} type="button" role="tab" aria-selected={tab === id} onClick={() => setTab(id)}>
              {label}
              <span className="count">{count}</span>
            </button>
          ))}
        </div>
        {tab === "running" ? <RunningView live={live} /> : <FinishedTable ended={ended} />}
      </div>
    </div>
  );
}
