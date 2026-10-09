// How often running machines are saved whole, and how many automatic copies to keep.
import type { Settings } from "../../api/generated/Settings";
import { Segmented } from "../../components/Segmented";
import { Switch } from "../../components/Switch";
import type { SetSetting } from "./useDraft";

const every = (m: number) => (m === 0 ? "Off" : m < 60 ? `${m} min` : `${m / 60} h`);

export function AutoSnapshots({ s, set }: { s: Settings; set: SetSetting }) {
  const a = s.auto_snapshots;
  const change = (patch: Partial<typeof a>) => set("auto_snapshots", { ...a, ...patch });
  return (
    <>
      <p className="lede">
        Running machines are saved whole (files, installed packages, Claude's conversation), so you can go back to any
        point. Copies share unchanged blocks, so they take little space.
      </p>
      <div className="set">
        <span className="set-label">Snapshot every</span>
        <Segmented
          label="Snapshot every"
          value={a.every_min}
          options={[0, 5, 15, 30, 60, 120].map((m) => [m, every(m)])}
          onChange={(m) => change({ every_min: m })}
          wide
        />
        <p className="hint">A machine can have its own interval: in its ⋯ menu, under Automatic snapshots.</p>
      </div>
      <div className="pair">
        <label className="set">
          <span className="set-label">Keep per machine</span>
          <span className="unit">
            <input
              className="text-input"
              type="number"
              min={1}
              max={50}
              value={a.keep}
              onChange={(e) => change({ keep: Number(e.target.value) })}
            />
            latest automatic snapshots
          </span>
          <span className="hint">Older automatic ones are deleted. Snapshots you take yourself never are.</span>
        </label>
        <Switch
          on={a.before_close}
          onChange={(on) => change({ before_close: on })}
          label="Snapshot before closing"
          hint="The machine as it was when you closed it, in case you need it again."
        />
      </div>
    </>
  );
}
