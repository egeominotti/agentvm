// This Mac: VMs running, memory promised to them, CPU they use, what Claude would cost.
import type { Status } from "../../api/generated/Status";
import type { TaskDto } from "../../api/generated/TaskDto";
import { gb, money } from "../../lib/format";

function Gauge({ label, value, pct, hot }: { label: string; value: string; pct: number; hot?: boolean }) {
  return (
    <div className={`gauge${hot ? " hot" : ""}`}>
      <div className="row">
        <span>{label}</span>
        <b className="num">{value}</b>
      </div>
      <div className="track" aria-hidden="true">
        <i style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} />
      </div>
    </div>
  );
}

export function HostGauges({ status: s, tasks }: { status: Status; tasks: TaskDto[] }) {
  const live = tasks.filter((t) => t.metrics && t.status.state === "running");
  const cpu = live.reduce((n, t) => n + (t.metrics?.cpu_pct ?? 0) * (t.metrics?.cpus ?? 0), 0) / s.host.cpus;
  const spent = tasks.reduce((n, t) => n + (t.usage?.cost_usd ?? 0), 0);
  return (
    <section className="host" aria-label="This Mac">
      <Gauge
        label="VMs"
        value={`${s.running} / ${s.concurrency}`}
        pct={(100 * s.running) / s.concurrency}
        hot={s.running >= s.concurrency}
      />
      <Gauge
        label="Memory reserved"
        value={`${gb(s.ram_committed_mb)} / ${gb(s.host.ram_mb)}`}
        pct={(100 * s.ram_committed_mb) / s.host.ram_mb}
        hot={s.ram_committed_mb > s.host.ram_mb * 0.85}
      />
      <Gauge label="CPU in VMs" value={`${cpu.toFixed(0)}%`} pct={cpu} hot={cpu > 85} />
      <div
        className="gauge spend"
        title="What these agents would cost at API prices; with a subscription it counts against its limits"
      >
        <div className="row">
          <span>Claude usage</span>
          <b className="num">{money(spent)}</b>
        </div>
      </div>
    </section>
  );
}
