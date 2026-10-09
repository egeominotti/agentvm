// The sidebar: new VM, the sections, the machines that are running, and this Mac's load.
import { useStatus, useTasks } from "../../api/queries";
import { Icon, type IconName } from "../../components/Icon";
import { isEnded } from "../../lib/task";
import type { Route } from "../router";
import { HostGauges } from "./HostGauges";
import { MachineList } from "./MachineList";

const SECTIONS: [Route["name"], string, string, IconName][] = [
  ["wall", "#/wall", "Machines", "machines"],
  ["snapshots", "#/snapshots", "Snapshots", "snapshots"],
  ["settings", "#/settings", "Settings", "settings"],
];

export function Sidebar({ route }: { route: Route }) {
  const tasks = useTasks().data ?? [];
  const status = useStatus().data;
  const live = tasks.filter((t) => !isEnded(t)).length;
  const focused = route.name === "machine" ? route.id : null;
  return (
    <aside className="side" aria-label="Navigation">
      <a className="brand" href="#/wall">
        <img src={`${import.meta.env.BASE_URL}logo.svg`} alt="" width="18" height="18" />
        <span>agentvm</span>
      </a>
      <button
        className="new-vm"
        type="button"
        title="New VM (⌘K)"
        onClick={() => window.dispatchEvent(new Event("agentvm:new-vm"))}
      >
        <Icon name="plus" />
        <span>New VM</span>
        <kbd>⌘K</kbd>
      </button>
      {status && !status.token ? (
        <a className="alert" href="#/settings">
          Claude token missing
        </a>
      ) : null}
      {status && !status.golden ? (
        <a className="alert" href="#/settings">
          VM image missing
        </a>
      ) : null}
      <nav className="nav" aria-label="Sections">
        {SECTIONS.map(([name, href, label, icon]) => (
          <a
            key={name}
            href={href}
            aria-current={route.name === name || (name === "wall" && focused) ? "page" : undefined}
          >
            <Icon name={icon} />
            {label}
            {name === "wall" && live ? <span className="count num">{live}</span> : null}
          </a>
        ))}
      </nav>
      <MachineList tasks={tasks} focused={focused} />
      {status ? <HostGauges status={status} tasks={tasks} /> : null}
    </aside>
  );
}
