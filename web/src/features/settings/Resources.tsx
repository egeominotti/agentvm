// How many VMs run at once and what each gets, against this Mac's cores and memory.
import type { HostLimits } from "../../api/generated/HostLimits";
import type { Settings } from "../../api/generated/Settings";
import { gb } from "../../lib/format";
import { memoryChoices } from "../../lib/memory";
import { Page, Panel, Row } from "./kit";
import type { SetSetting } from "./useDraft";

/** Memory kept for macOS, never given to VMs. */
const RESERVE_MB = 8192;

export function Resources({ s, set, limits }: { s: Settings; set: SetSetting; limits: HostLimits }) {
  const total = limits.ram_mb;
  const used = s.max_vms * s.memory_mb;
  const fit = Math.max(1, Math.floor((total - RESERVE_MB) / s.memory_mb));
  const over = used > total - RESERVE_MB;
  return (
    <Page
      title="Resources"
      description={`What this Mac gives its VMs: ${limits.cpus} cores and ${gb(total)} of memory, ${gb(RESERVE_MB)} of it always kept for macOS.`}
    >
      <Panel title="Each VM" description="New VMs get these; running ones keep theirs.">
        <Row label="vCPUs" htmlFor="set-cpus" description="VMs share the cores, so all together they can have more.">
          <Slider id="set-cpus" value={s.cpus} min={1} max={limits.cpus} onChange={(v) => set("cpus", v)} />
        </Row>
        <Row label="Memory" htmlFor="set-memory" description="Reserved for each running VM.">
          <select
            id="set-memory"
            className="text-input"
            value={s.memory_mb}
            onChange={(e) => set("memory_mb", Number(e.target.value))}
          >
            {memoryChoices(total, s.memory_mb).map((m) => (
              <option key={m} value={m}>
                {gb(m)}
              </option>
            ))}
          </select>
        </Row>
      </Panel>
      <Panel title="Capacity" description="More launches than this wait in a queue, in order.">
        <Row label="VMs at the same time" htmlFor="set-vms" description="Takes effect at once.">
          <Slider
            id="set-vms"
            value={s.max_vms}
            min={1}
            max={Math.min(64, Math.max(limits.cpus * 2, 16))}
            onChange={(v) => set("max_vms", v)}
          />
        </Row>
        <Row label="Memory at full load" wide>
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
                for VMs
              </span>
              <span>{gb(RESERVE_MB)} for macOS</span>
              <span className="fit">
                {over
                  ? `Too much: ${fit} VMs of ${gb(s.memory_mb)} fit without swapping`
                  : `Fits: up to ${fit} VMs of ${gb(s.memory_mb)}`}
              </span>
            </div>
          </div>
        </Row>
      </Panel>
    </Page>
  );
}

type SliderProps = { id: string; value: number; min: number; max: number; onChange: (v: number) => void };

function Slider({ id, value, min, max, onChange }: SliderProps) {
  return (
    <span className="set-slider">
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      <output htmlFor={id}>{value}</output>
    </span>
  );
}
