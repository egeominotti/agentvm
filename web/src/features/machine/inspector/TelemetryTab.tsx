// The VM's numbers: live or over its whole life, said to be stale when they stop coming.
import { useState } from "react";
import type { TaskDto } from "../../../api/generated/TaskDto";
import { useTelemetry } from "../../../api/queries";
import { BLUE, LineChart, ORANGE } from "../../../components/LineChart";
import { gb, rate } from "../../../lib/format";
import { isEnded } from "../../../lib/task";
import { Facts, Processes } from "./Facts";

const RANGES = [
  ["5m", "5 min"],
  ["1h", "1 hour"],
  ["all", "Whole life"],
] as const;
/** Numbers older than this are shown as stale, not as live. */
const STALE_S = 5;

export function TelemetryTab({ task: t }: { task: TaskDto }) {
  const ended = isEnded(t);
  // A closed VM has no live numbers: its whole life is what is left to show.
  const [range, setRange] = useState<string>(ended ? "all" : "5m");
  const points = useTelemetry(t.id, range, !ended).data?.points ?? [];
  const times = points.map((p) => p.at);
  const stale = !ended && t.metrics_age_s != null && t.metrics_age_s > STALE_S;
  const diskMeasured = points.length === 0 || points.some((p) => p.disk_read_bps != null);
  const memTop = Math.max(t.memory_mb, ...points.map((p) => p.mem_used_mb));
  return (
    <div className="telemetry">
      {ended ? <p className="hint">This VM is closed: this is its history.</p> : null}
      {stale ? (
        <p className="stale">
          No new numbers for {Math.round(t.metrics_age_s ?? 0)} s: the VM may be very busy or stuck. These are the last
          ones.
        </p>
      ) : null}
      <div className="seg" role="tablist" aria-label="Time range">
        {RANGES.map(([key, label]) => (
          <button key={key} type="button" role="tab" aria-selected={range === key} onClick={() => setRange(key)}>
            {label}
          </button>
        ))}
      </div>
      <LineChart
        title="CPU"
        times={times}
        max={100}
        format={(v) => `${v.toFixed(0)}%`}
        series={[{ name: "CPU", color: BLUE, values: points.map((p) => p.cpu_pct) }]}
      />
      <LineChart
        title="Memory"
        times={times}
        max={memTop}
        format={gb}
        reference={{ value: t.memory_mb, label: `given ${gb(t.memory_mb)}` }}
        series={[
          { name: "Used", color: BLUE, values: points.map((p) => p.mem_used_mb) },
          { name: "Allowed now", color: ORANGE, values: points.map((p) => p.mem_limit_mb) },
        ]}
      />
      <LineChart
        title="Disk I/O"
        times={times}
        format={rate}
        empty={diskMeasured ? "No data yet" : "Not measured: this VM started before disk I/O was collected"}
        series={
          diskMeasured
            ? [
                { name: "Read", color: BLUE, values: points.map((p) => p.disk_read_bps) },
                { name: "Write", color: ORANGE, values: points.map((p) => p.disk_write_bps) },
              ]
            : []
        }
      />
      <LineChart
        title="Network"
        times={times}
        format={rate}
        series={[
          { name: "In", color: BLUE, values: points.map((p) => p.net_rx_bps) },
          { name: "Out", color: ORANGE, values: points.map((p) => p.net_tx_bps) },
        ]}
      />
      {!ended && t.metrics ? <Processes m={t.metrics} /> : null}
      <h3 className="tab-heading">This machine</h3>
      <Facts task={t} />
    </div>
  );
}
