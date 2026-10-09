// The running machines as a table: dense, for many at once, with what can be done on each.
import type { TaskDto } from "../../api/generated/TaskDto";
import { go } from "../../app/router";
import { Button, IconButton } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Menu, MenuItem } from "../../components/Menu";
import { StatusMark } from "../../components/StatusMark";
import { gb, money, repoName } from "../../lib/format";
import { age, shortId, titleOf } from "../../lib/task";
import { useMachineActions } from "../machine/actions";

export function MachineTable({ tasks }: { tasks: TaskDto[] }) {
  return (
    <div className="data-table cols-machines" role="table" aria-label="Running machines">
      <div className="dt-tr dt-th" role="row">
        <span role="columnheader">Status</span>
        <span role="columnheader">Machine</span>
        <span role="columnheader" className="num wide-only">
          CPU
        </span>
        <span role="columnheader" className="num wide-only">
          Memory
        </span>
        <span role="columnheader" className="wide-only">
          Open on this Mac
        </span>
        <span role="columnheader" className="num wide-only">
          Claude
        </span>
        <span role="columnheader" className="num wide-only">
          Up
        </span>
        <span role="columnheader" aria-label="Actions" />
      </div>
      {tasks.map((t) => (
        <MachineRow key={t.id} t={t} />
      ))}
    </div>
  );
}

function MachineRow({ t }: { t: TaskDto }) {
  const actions = useMachineActions(t.id);
  const m = t.metrics;
  const ready = t.status.state === "running" && t.ready;
  const port = t.ports.find((p) => p.kind === "http") ?? t.ports[0];
  return (
    <div className="dt-tr dt-row" role="row">
      <span role="cell">
        <StatusMark task={t} label />
      </span>
      <span role="cell" className="dt-name">
        <a href={`#/vm/${t.id}`} title={`Open: ${titleOf(t)}`}>
          <b>{titleOf(t)}</b>
        </a>
        <span className="muted">
          {repoName(t.repo)} · #{shortId(t)}
        </span>
      </span>
      <span role="cell" className="num wide-only">
        {m ? `${m.cpu_pct.toFixed(0)}%` : "–"}
      </span>
      <span role="cell" className="num wide-only">
        {m ? gb(m.mem_used_mb) : "–"}
      </span>
      <span role="cell" className="wide-only">
        {port?.url ? (
          <a className="port-chip" href={port.url} target="_blank" rel="noopener" title={`Opens ${port.url}`}>
            :{port.port}
            {t.ports.length > 1 ? ` +${t.ports.length - 1}` : ""}
          </a>
        ) : port ? (
          <span className="port-chip">:{port.port}</span>
        ) : (
          <span className="muted">–</span>
        )}
      </span>
      <span role="cell" className="num wide-only muted">
        {t.usage?.cost_usd ? money(t.usage.cost_usd) : "–"}
      </span>
      <span role="cell" className="num wide-only muted">
        {age(t)}
      </span>
      <span role="cell" className="dt-actions">
        <Button size="sm" onClick={() => go(`#/vm/${t.id}`)}>
          Open
        </Button>
        <Menu
          trigger={
            <IconButton aria-label={`More for ${titleOf(t)}`}>
              <Icon name="more" />
            </IconButton>
          }
        >
          <MenuItem icon="save" disabled={!ready || actions.save.isPending} onSelect={() => actions.save.mutate()}>
            Save to its branch
          </MenuItem>
          <MenuItem
            icon="snapshot"
            disabled={!ready || !t.interactive || actions.snapshot.isPending}
            onSelect={() => actions.snapshot.mutate()}
          >
            Take a snapshot
          </MenuItem>
          <MenuItem
            icon="power"
            danger
            confirm="Save and close it?"
            disabled={!ready || actions.close.isPending}
            onSelect={() => actions.close.mutate()}
          >
            Close
          </MenuItem>
        </Menu>
      </span>
    </div>
  );
}
