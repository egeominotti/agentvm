// The machine's model, Claude Code version and resources, set from the Settings and changeable
// for this launch; with what still fits in this Mac's memory.
import { useGolden, useReleases, useSettings, useStatus } from "../../api/queries";
import { gb } from "../../lib/format";
import { memoryChoices } from "../../lib/memory";
import { MODELS } from "../../lib/models";

export type Choice = { model: string; version: string; cpus: number; memoryMb: number };

/** Kept for macOS, as the server's scheduler does. */
const RESERVED_MB = 8192;

export function Options({ value: v, onChange }: { value: Choice; onChange: (c: Choice) => void }) {
  const settings = useSettings().data;
  const status = useStatus().data;
  const golden = useGolden().data;
  const releases = useReleases().data;
  const hostCpus = settings?.limits.cpus ?? v.cpus;
  const hostRam = settings?.limits.ram_mb ?? 0;
  const cpus = [...new Set([1, 2, 4, 6, 8, 12, 16, hostCpus])].filter((n) => n <= hostCpus).toSorted((a, b) => a - b);
  const memory = memoryChoices(hostRam, v.memoryMb);
  const set = (part: Partial<Choice>) => onChange({ ...v, ...part });
  const free = status ? status.host.ram_mb - RESERVED_MB - status.ram_committed_mb : null;
  const fits = free == null ? null : Math.floor(free / v.memoryMb);
  // How many start now: a free slot and the memory for it.
  const slots = status ? Math.max(0, status.concurrency - status.running) : null;
  const now = fits == null || slots == null ? null : Math.min(fits, slots);
  return (
    <div className="options">
      <label>
        <span>Model</span>
        <select value={v.model} onChange={(e) => set({ model: e.target.value })}>
          {MODELS.map(([id, name]) => (
            <option key={id} value={id}>
              {name}
            </option>
          ))}
          {MODELS.some(([id]) => id === v.model) ? null : <option value={v.model}>{v.model}</option>}
        </select>
      </label>
      <label>
        <span>Claude Code</span>
        <select value={v.version} onChange={(e) => set({ version: e.target.value })}>
          <option value="">{golden?.claude_version ? `Image's (${golden.claude_version})` : "Image's version"}</option>
          {/* The four newest releases: enough to step back from a bad one. */}
          {releases?.versions.slice(0, 4).map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span>vCPUs</span>
        <select value={v.cpus} onChange={(e) => set({ cpus: Number(e.target.value) })}>
          {cpus.map((n) => (
            <option key={n} value={n}>
              {n === hostCpus ? `${n} (all cores)` : n}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span>Memory</span>
        <select value={v.memoryMb} onChange={(e) => set({ memoryMb: Number(e.target.value) })}>
          {memory.map((m) => (
            <option key={m} value={m}>
              {gb(m)}
            </option>
          ))}
        </select>
      </label>
      <p className={`res-hint${now === 0 ? " over" : ""}`}>
        {now == null
          ? ""
          : now === 0
            ? `It waits in the queue: ${
                slots != null && slots < Math.max(fits ?? 0, 0)
                  ? `${status?.concurrency} run at a time`
                  : `no memory free for ${gb(v.memoryMb)}`
              } right now.`
            : `${gb(Math.max(free ?? 0, 0))} free for VMs: room for ${fits} like this.`}
        {v.cpus >= hostCpus && hostCpus > 1 ? " Every VM shares this Mac's cores." : ""}
      </p>
    </div>
  );
}
