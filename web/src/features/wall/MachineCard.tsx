// A running machine on the wall: its state, name, a live preview, and the numbers that matter.
import type { TaskDto } from "../../api/generated/TaskDto";
import { useStatus } from "../../api/queries";
import { StatusMark } from "../../components/StatusMark";
import { useToast } from "../../components/Toast";
import { gb, money, tokens } from "../../lib/format";
import { age, shortId, statusOf, titleOf } from "../../lib/task";
import { bootSteps, setupFailed } from "../machine/boot";
import { Preview } from "./Preview";

export function MachineCard({ task: t }: { task: TaskDto }) {
  const say = useToast();
  const { tone } = statusOf(t);
  const ready = t.status.state === "running" && t.ready;
  const m = t.metrics;
  const u = t.usage;
  const port = t.ports.find((p) => p.kind === "http") ?? t.ports[0];
  return (
    <article className={`card tone-${tone}`}>
      <a className="card-link" href={`#/vm/${t.id}`} title={`Open: ${titleOf(t)}\n${t.repo}`}>
        <header className="card-head">
          <StatusMark task={t} label />
          <span className="card-title">{titleOf(t)}</span>
          <span className="card-id">#{shortId(t)}</span>
        </header>
        <div className="card-body">
          {!t.interactive ? (
            <Note title="An automatic task" text="No terminal: open it to follow Claude's work." />
          ) : ready ? (
            <Preview id={t.id} />
          ) : (
            <CardBoot task={t} />
          )}
        </div>
      </a>
      <footer className="card-foot">
        <span>
          <b>{m ? `${m.cpu_pct.toFixed(0)}%` : "—"}</b> CPU
        </span>
        <span>
          <b>{m ? gb(m.mem_used_mb) : "—"}</b> RAM
        </span>
        {port?.url ? (
          <a className="port-chip" href={port.url} target="_blank" rel="noopener" title={`Opens ${port.url}`}>
            :{port.port}
            {t.ports.length > 1 ? ` +${t.ports.length - 1}` : ""}
          </a>
        ) : port ? (
          // Not a web page: its address on the Mac, to connect a client to.
          <button
            type="button"
            className="port-chip"
            title={`Not a web page: copy localhost:${port.host_port}`}
            onClick={() =>
              navigator.clipboard
                .writeText(`localhost:${port.host_port}`)
                .then(() => say(`Copied localhost:${port.host_port}`))
            }
          >
            :{port.port}
            {t.ports.length > 1 ? ` +${t.ports.length - 1}` : ""}
          </button>
        ) : null}
        {setupFailed(t) ? (
          <span className="card-warn" title="The repository's .agentvm/setup.sh failed">
            setup failed
          </span>
        ) : null}
        <span className="card-spend">
          {u?.output_tokens ? `${money(u.cost_usd)} · ${tokens(u.input_tokens + u.output_tokens)} tokens · ` : ""}
          {age(t)}
        </span>
      </footer>
    </article>
  );
}

function CardBoot({ task: t }: { task: TaskDto }) {
  const concurrency = useStatus().data?.concurrency;
  const { steps, current } = bootSteps(t);
  if (t.status.state === "queued") {
    return (
      <Note
        title="Waiting for a free slot"
        text={`${concurrency ?? "A few"} VMs run at a time; it starts when one closes. Settings › Resources changes it.`}
      />
    );
  }
  const now = current < steps.length ? steps[current]?.label : "Opening the terminal";
  return (
    <div className="card-boot">
      <b>{now}…</b>
      <div className="boot-bar">
        <i style={{ width: `${(100 * current) / steps.length}%` }} />
      </div>
      <span>
        Step {Math.min(current + 1, steps.length)} of {steps.length}
      </span>
    </div>
  );
}

const Note = ({ title, text }: { title: string; text: string }) => (
  <div className="card-note">
    <b>{title}</b>
    <span>{text}</span>
  </div>
);
