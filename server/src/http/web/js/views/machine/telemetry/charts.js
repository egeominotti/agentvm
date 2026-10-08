/* The VM's charts from its telemetry points: CPU, memory, disk I/O, network. */
import { gb, rate } from "../../../format.js";
import { lineChart } from "../../../ui/line-chart.js";

// Categorical slots 1 and 2 of the reference palette, dark steps (validated on the panel's surface).
const BLUE = "#3987e5", ORANGE = "#d95926";

/** `points`: TelemetrySample[] (oldest first); `t`: the task (for the configured memory). */
export function charts(points, t) {
  const times = points.map(p => p.at);
  const col = key => points.map(p => p[key] ?? null);
  const unmeasured = "Not measured: this VM started before disk I/O was collected";
  const disk = col("disk_read_bps").some(v => v != null);
  return [
    lineChart({ title: "CPU", times, max: 100, format: v => `${v.toFixed(0)}%`,
      series: [{ name: "CPU", color: BLUE, values: col("cpu_pct") }] }),
    // The memory the VM was given is the natural top: used and allowed both live under it.
    lineChart({ title: "Memory", times, format: v => gb(v), max: Math.max(t.memory_mb, ...col("mem_used_mb").filter(v => v != null)), reference: { value: t.memory_mb, label: `given ${gb(t.memory_mb)}` },
      series: [{ name: "Used", color: BLUE, values: col("mem_used_mb") }, { name: "Allowed now", color: ORANGE, values: col("mem_limit_mb") }] }),
    lineChart({ title: "Disk I/O", times, format: rate, empty: disk ? "No data yet" : unmeasured,
      series: disk ? [{ name: "Read", color: BLUE, values: col("disk_read_bps") }, { name: "Write", color: ORANGE, values: col("disk_write_bps") }] : [] }),
    lineChart({ title: "Network", times, format: rate,
      series: [{ name: "In", color: BLUE, values: col("net_rx_bps") }, { name: "Out", color: ORANGE, values: col("net_tx_bps") }] }),
  ];
}
