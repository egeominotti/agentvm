// The running machines: found by title, repository or branch, filtered by what they are doing,
// shown as live cards or as a table (the choice is kept in this browser).
import { useState } from "react";
import type { TaskDto } from "../../api/generated/TaskDto";
import { Button } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Segmented } from "../../components/Segmented";
import { openLauncher } from "../launcher/open";
import { filterLive, type LiveKind } from "./filter";
import { MachineCard } from "./MachineCard";
import { MachineTable } from "./MachineTable";

type View = "grid" | "list";
const VIEW_KEY = "agentvm.machines.view";

const KINDS: [LiveKind | "all", string][] = [
  ["all", "All"],
  ["waiting", "Waiting for you"],
  ["working", "Working"],
  ["starting", "Starting"],
];

function storedView(): View {
  try {
    return localStorage.getItem(VIEW_KEY) === "list" ? "list" : "grid";
  } catch {
    return "grid";
  }
}

export function RunningView({ live }: { live: TaskDto[] }) {
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<LiveKind | "all">("all");
  const [view, setView] = useState<View>(storedView);
  const shown = filterLive(live, query, kind);
  if (!live.length) {
    return (
      <div className="empty-panel">
        <Icon name="machines" />
        <b>Nothing running</b>
        <p>Launch a VM from a folder or a git link: Claude Code opens in it in about two seconds.</p>
        <Button variant="primary" onClick={openLauncher}>
          <Icon name="plus" />
          New VM
        </Button>
      </div>
    );
  }
  return (
    <>
      <div className="data-toolbar">
        <label className="data-search">
          <Icon name="search" />
          <input
            className="text-input"
            type="search"
            placeholder="Search by title, repository or branch"
            aria-label="Search machines"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <Segmented label="Doing" value={kind} options={KINDS} onChange={setKind} />
        <span className="tb-gap" />
        <Segmented
          label="View"
          value={view}
          options={[
            ["grid", "Grid"],
            ["list", "List"],
          ]}
          onChange={(v) => {
            setView(v);
            try {
              localStorage.setItem(VIEW_KEY, v);
            } catch {
              // Private windows: the choice lasts for this page only.
            }
          }}
        />
      </div>
      {!shown.length ? (
        <p className="msg">No running machine matches.</p>
      ) : view === "grid" ? (
        <div className="cards">
          {shown.map((t) => (
            <MachineCard key={t.id} task={t} />
          ))}
        </div>
      ) : (
        <MachineTable tasks={shown} />
      )}
    </>
  );
}
