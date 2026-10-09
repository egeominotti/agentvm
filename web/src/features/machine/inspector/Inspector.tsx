// The panel beside a machine: its telemetry, Claude's history and its diagnostics, one at a
// time. Its width follows a drag of its edge and is remembered in this browser.
import * as Tabs from "@radix-ui/react-tabs";
import { type PointerEvent as ReactPointerEvent, useState } from "react";
import type { TaskDto } from "../../../api/generated/TaskDto";
import { isEnded } from "../../../lib/task";
import { ClaudeTab } from "./ClaudeTab";
import { DiagnosticsTab } from "./DiagnosticsTab";
import { TelemetryTab } from "./TelemetryTab";

export type InspectorTab = "telemetry" | "claude" | "diagnostics";

const WIDTH_KEY = "agentvm.inspector.width";
const MIN = 280;
const MAX = 640;

function storedWidth(): number {
  try {
    const w = Number(localStorage.getItem(WIDTH_KEY));
    return w >= MIN && w <= MAX ? w : 340;
  } catch {
    return 340;
  }
}

type Props = { task: TaskDto; tab: InspectorTab; onTab: (t: InspectorTab) => void };

export function Inspector({ task: t, tab, onTab }: Props) {
  const [width, setWidth] = useState(storedWidth);
  const live = !isEnded(t);

  const drag = (e: ReactPointerEvent<HTMLDivElement>) => {
    const start = e.clientX;
    const from = width;
    let last = from;
    const move = (ev: PointerEvent) => {
      last = Math.max(MIN, Math.min(MAX, from + start - ev.clientX));
      setWidth(last);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      try {
        localStorage.setItem(WIDTH_KEY, String(last));
      } catch {
        // Not remembered: it still works.
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  return (
    <aside className="inspector" style={{ width }} aria-label="Machine details">
      <div className="inspector-edge" onPointerDown={drag} title="Drag to resize" />
      <Tabs.Root value={tab} onValueChange={(v) => onTab(v as InspectorTab)} className="inspector-tabs">
        <Tabs.List className="tabs" aria-label="Details">
          <Tabs.Trigger value="telemetry">Telemetry</Tabs.Trigger>
          <Tabs.Trigger value="claude">Claude</Tabs.Trigger>
          <Tabs.Trigger value="diagnostics">Diagnostics</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="telemetry" className="tab-body">
          <TelemetryTab task={t} />
        </Tabs.Content>
        <Tabs.Content value="claude" className="tab-body">
          <ClaudeTab id={t.id} live={live} />
        </Tabs.Content>
        <Tabs.Content value="diagnostics" className="tab-body">
          <DiagnosticsTab id={t.id} />
        </Tabs.Content>
      </Tabs.Root>
    </aside>
  );
}
