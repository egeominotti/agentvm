// The machine's busiest processes and its facts.
import type { TaskDto } from "../../../api/generated/TaskDto";
import type { VmMetrics } from "../../../api/generated/VmMetrics";
import { gb, repoName } from "../../../lib/format";
import { modelLabel } from "../../../lib/models";
import { age, statusOf } from "../../../lib/task";

export function Processes({ m }: { m: VmMetrics }) {
  return (
    <div className="procs">
      <ProcList title="Busiest (CPU)" rows={m.top} value={(p) => `${p.cpu_pct.toFixed(0)}% · ${gb(p.mem_mb)}`} />
      <ProcList title="Largest (memory)" rows={m.top_mem.length ? m.top_mem : null} value={(p) => gb(p.mem_mb)} />
    </div>
  );
}

type Proc = VmMetrics["top"][number];

function ProcList({ title, rows, value }: { title: string; rows: Proc[] | null; value: (p: Proc) => string }) {
  return (
    <div>
      <h3 className="tab-heading">{title}</h3>
      {rows == null ? (
        <p className="hint">—</p>
      ) : rows.length ? (
        rows.map((p) => (
          <div className="proc" key={p.name}>
            <span>{p.name}</span>
            <span>{value(p)}</span>
          </div>
        ))
      ) : (
        <p className="hint">Idle</p>
      )}
    </div>
  );
}

export function Facts({ task: t }: { task: TaskDto }) {
  const rows: [string, string, string?][] = [
    ["State", statusOf(t).label],
    ["Model", modelLabel(t.model)],
    ["Claude Code", t.claude_version ?? "image version"],
    ["Resources", `${t.cpus} vCPUs, ${gb(t.memory_mb)}`],
    ["Repository", repoName(t.repo), t.repo],
    ["Branch", t.branch, t.branch],
    ["From commit", t.base_sha.slice(0, 10)],
    ["Age", age(t)],
    ["Id", t.id, t.id],
  ];
  return (
    <dl className="facts">
      {rows.map(([k, v, title]) => (
        <div key={k}>
          <dt>{k}</dt>
          <dd title={title}>{v}</dd>
        </div>
      ))}
    </dl>
  );
}
