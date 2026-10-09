// The machine's bar: what it is and its state, the two sessions, and the two things people do
// with a running machine (Save, Close). Everything else waits in the "⋯" menu.
import type { TaskDto } from "../../api/generated/TaskDto";
import { useSettings } from "../../api/queries";
import { Button, IconButton } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Menu, MenuItem, MenuLabel, MenuSeparator } from "../../components/Menu";
import { StatusMark } from "../../components/StatusMark";
import { isEnded, shortId, titleOf } from "../../lib/task";
import type { MachineActions } from "./actions";
import { SessionTabs } from "./SessionTabs";
import type { Sessions } from "./useSessions";

const INTERVALS = [0, 5, 15, 30, 60, 120];
const every = (m: number) => (m === 0 ? "Off" : m < 60 ? `Every ${m} min` : `Every ${m / 60} h`);

type Props = {
  task: TaskDto;
  sessions: Sessions;
  actions: MachineActions;
  inspector: boolean;
  onInspector: () => void;
};

export function Toolbar({ task: t, sessions, actions: a, inspector, onInspector }: Props) {
  const ended = isEnded(t);
  const running = t.status.state === "running";
  const terminal = t.interactive && !ended;
  const fallback = useSettings().data?.settings.auto_snapshots.every_min ?? 30;
  const busy = a.save.isPending || a.close.isPending;
  return (
    <header className="toolbar">
      <StatusMark task={t} label />
      <h1 className="tb-title" title={`${t.prompt || "(no first task)"}\n${t.repo}`}>
        {titleOf(t)}
      </h1>
      <span className="tb-id" title={t.id}>
        #{shortId(t)}
      </span>
      {terminal ? <SessionTabs s={sessions} /> : null}
      <span className="tb-gap" />
      {terminal ? (
        <>
          <Button
            disabled={!running || busy}
            onClick={() => a.save.mutate()}
            title={`Copy the VM's commits to the branch ${t.branch} in your repository. The VM keeps running.`}
          >
            <Icon name="save" />
            {a.save.isPending ? "Saving…" : "Save"}
          </Button>
          <Button
            variant="primary"
            disabled={!running || busy}
            onClick={() => a.close.mutate()}
            title="Save, then shut the VM down"
          >
            <Icon name="power" />
            {a.close.isPending ? "Closing…" : "Close"}
          </Button>
        </>
      ) : null}
      <Menu
        trigger={
          <IconButton aria-label="More actions" title="More actions">
            <Icon name="more" />
          </IconButton>
        }
      >
        {terminal ? (
          <>
            <MenuItem icon="snapshot" disabled={!running || a.snapshot.isPending} onSelect={() => a.snapshot.mutate()}>
              Take a snapshot now
            </MenuItem>
            <MenuLabel>Automatic snapshots</MenuLabel>
            <MenuItem onSelect={() => a.autoSnapshots.mutate(null)}>
              <Check on={t.auto_snapshot_min == null} />
              As in Settings ({every(fallback).toLowerCase()})
            </MenuItem>
            {INTERVALS.map((m) => (
              <MenuItem key={m} onSelect={() => a.autoSnapshots.mutate(m)}>
                <Check on={t.auto_snapshot_min === m} />
                {every(m)}
              </MenuItem>
            ))}
            <MenuSeparator />
          </>
        ) : null}
        <MenuItem icon="branch" onSelect={() => navigator.clipboard.writeText(t.branch)}>
          Copy branch name
        </MenuItem>
        <MenuItem icon="cloud-up" disabled={a.push.isPending} onSelect={() => a.push.mutate()}>
          Push branch to origin
        </MenuItem>
        {ended ? (
          <MenuItem
            icon="trash"
            danger
            confirm="Delete its logs and Claude's history? Its branch stays"
            onSelect={() => a.remove.mutate()}
          >
            Delete this machine
          </MenuItem>
        ) : (
          <MenuItem icon="stop" danger confirm="Power off now? Its disk is kept" onSelect={() => a.stop.mutate()}>
            Force stop
          </MenuItem>
        )}
      </Menu>
      <IconButton
        aria-pressed={inspector}
        aria-label="Details panel"
        title="Telemetry, Claude's history and diagnostics"
        onClick={onInspector}
      >
        <Icon name="panel" />
      </IconButton>
    </header>
  );
}

const Check = ({ on }: { on: boolean }) => <span className={`check${on ? " on" : ""}`} aria-hidden="true" />;
