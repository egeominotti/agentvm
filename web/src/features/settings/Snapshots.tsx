// Automatic snapshots of running terminals: how often, how many kept, one before closing.
import type { Settings } from "../../api/generated/Settings";
import { Page, Panel, Row, Toggle } from "./kit";
import type { SetSetting } from "./useDraft";

const every = (m: number) =>
  m === 0 ? "Never" : m < 60 ? `Every ${m} minutes` : m === 60 ? "Every hour" : `Every ${m / 60} hours`;

export function Snapshots({ s, set }: { s: Settings; set: SetSetting }) {
  const a = s.auto_snapshots;
  const change = (patch: Partial<typeof a>) => set("auto_snapshots", { ...a, ...patch });
  return (
    <Page
      title="Snapshots"
      description="Running VMs saved whole (files, installed packages, Claude's conversation), compressed into chunks they share: a snapshot usually adds a few megabytes."
    >
      <Panel title="Automatic snapshots" description="A VM can have its own interval, in its ⋯ menu.">
        <Row label="Snapshot running VMs" htmlFor="set-every">
          <select
            id="set-every"
            className="text-input"
            value={a.every_min}
            onChange={(e) => change({ every_min: Number(e.target.value) })}
          >
            {[...new Set([0, 5, 15, 30, 60, 120, 240, a.every_min])]
              .toSorted((x, y) => x - y)
              .map((m) => (
                <option key={m} value={m}>
                  {every(m)}
                </option>
              ))}
          </select>
        </Row>
        <Row
          label="Keep per VM"
          htmlFor="set-keep"
          description="Older automatic ones are deleted. Snapshots you take yourself never are."
        >
          <span className="set-number">
            <input
              id="set-keep"
              className="text-input"
              type="number"
              min={1}
              max={50}
              value={a.keep}
              onChange={(e) => change({ keep: Number(e.target.value) })}
            />
            latest
          </span>
        </Row>
        <Row label="Before closing" description="One more snapshot as a VM closes, in case you need it again.">
          <Toggle label="Snapshot before closing" on={a.before_close} onChange={(on) => change({ before_close: on })} />
        </Row>
      </Panel>
    </Page>
  );
}
