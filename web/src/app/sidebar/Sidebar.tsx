// The sidebar: new VM, the sections, the machines that are running, and this Mac's load.

import { useAgentCatalog, useSettings, useStatus, useTasks } from "../../api/queries";
import { Icon, type IconName } from "../../components/Icon";
import { Logo } from "../../components/Logo";
import { openLauncher } from "../../features/launcher/open";
import { CLAUDE_AGENT } from "../../lib/agents";
import { selectedAccount } from "../../lib/accounts";
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
  const defaultAgent = useSettings().data?.settings.agent;
  const catalog = useAgentCatalog().data;
  const claudeConnected = catalog?.accounts?.find((a) => a.id === "claude")?.connected ?? status?.token;
  const live = tasks.filter((t) => !isEnded(t)).length;
  const focused = route.name === "machine" ? route.id : null;
  return (
    <aside className="side" aria-label="Navigation">
      <a className="brand" href="#/wall">
        <Logo />
        <span>agentvm</span>
      </a>
      <button className="new-vm" type="button" title="New VM (⌘K)" onClick={openLauncher}>
        <Icon name="plus" />
        <span>New VM</span>
        <kbd>⌘K</kbd>
      </button>
      {status && !claudeConnected && selectedAccount(defaultAgent ?? CLAUDE_AGENT) === "claude" ? (
        <a className="alert" href={catalog?.accounts ? "#/settings/accounts" : "#/settings/account"}>
          Claude account missing
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
            title={label}
          >
            <Icon name={icon} />
            <span className="nav-label">{label}</span>
            {name === "wall" && live ? <span className="count num">{live}</span> : null}
          </a>
        ))}
      </nav>
      <MachineList tasks={tasks} focused={focused} />
      {status ? <HostGauges status={status} tasks={tasks} /> : null}
    </aside>
  );
}
