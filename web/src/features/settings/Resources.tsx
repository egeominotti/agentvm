// How many VMs at once, and the vCPUs and memory of each, against this Mac's memory.
import type { HostLimits } from "../../api/generated/HostLimits";
import type { Settings } from "../../api/generated/Settings";
import { Segmented } from "../../components/Segmented";
import { gb } from "../../lib/format";
import { memoryChoices } from "../../lib/memory";
import type { SetSetting } from "./useDraft";

/** Memory kept for macOS, never given to VMs. */
const RESERVE_MB = 8192;

export function Resources({ s, set, limits }: { s: Settings; set: SetSetting; limits: HostLimits }) {
  const total = limits.ram_mb;
  const used = s.max_vms * s.memory_mb;
  const fit = Math.max(1, Math.floor((total - RESERVE_MB) / s.memory_mb));
  const over = used > total - RESERVE_MB;
  return (
    <>
      <p className="lede">
        What this Mac gives the VMs: {limits.cpus} cores, {gb(total)} of memory.
      </p>
      <div className="res">
        <Slider
          label="VMs at the same time"
          value={s.max_vms}
          min={1}
          max={Math.min(64, Math.max(limits.cpus * 2, 16))}
          onChange={(v) => set("max_vms", v)}
          hint="More launches wait in a queue. Takes effect at once."
        />
        <Slider
          label="vCPUs per VM"
          value={s.cpus}
          min={1}
          max={limits.cpus}
          onChange={(v) => set("cpus", v)}
          hint={`This Mac has ${limits.cpus} cores. VMs share them, so the total can exceed them.`}
        />
        <div className="set">
          <span className="set-label">Memory per VM</span>
          <Segmented
            label="Memory per VM"
            value={s.memory_mb}
            options={memoryChoices(total, s.memory_mb).map((m) => [m, gb(m)])}
            onChange={(v) => set("memory_mb", v)}
            wide
          />
          <p className="hint">Reserved for each running VM. New VMs get the new size.</p>
        </div>
      </div>
      <div className={`budget${over ? " over" : ""}`}>
        <div className="budget-bar" aria-hidden="true">
          <i className="vms" style={{ width: `${Math.min(100, (100 * used) / total)}%` }} />
          <i className="reserve" style={{ width: `${(100 * RESERVE_MB) / total}%` }} />
        </div>
        <div className="budget-legend">
          <span>
            <b>
              {s.max_vms} × {gb(s.memory_mb)} = {gb(used)}
            </b>{" "}
            for VMs at full load
          </span>
          <span>{gb(RESERVE_MB)} kept for macOS</span>
          <span className="fit">
            {over
              ? `Too much: at most ${fit} VMs of ${gb(s.memory_mb)} fit without swapping`
              : `Fits: up to ${fit} VMs of ${gb(s.memory_mb)}`}
          </span>
        </div>
      </div>
    </>
  );
}

type SliderProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
  hint: string;
};

function Slider({ label, value, min, max, onChange, hint }: SliderProps) {
  return (
    <label className="set slider">
      <span className="slider-head">
        <span className="set-label">{label}</span>
        <output>{value}</output>
      </span>
      <input type="range" min={min} max={max} value={value} onChange={(e) => onChange(Number(e.target.value))} />
      <span className="hint">{hint}</span>
    </label>
  );
}
