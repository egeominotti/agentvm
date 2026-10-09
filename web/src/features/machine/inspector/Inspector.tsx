// The panel beside a machine: its telemetry, Claude's history and its diagnostics, one at a
// time. Its width follows a drag of its edge and is remembered in this browser.
import * as Tabs from "@radix-ui/react-tabs";
import { type PointerEvent as ReactPointerEvent, useState } from "react";
import type { TaskDto } from "../../../api/generated/TaskDto";
import { IconButton } from "../../../components/Button";
import { Icon } from "../../../components/Icon";
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

function remember(width: number) {
  try {
    localStorage.setItem(WIDTH_KEY, String(width));
  } catch {
    // Not remembered: it still works.
  }
}

type Props = { task: TaskDto; tab: InspectorTab; onTab: (t: InspectorTab) => void; onClose: () => void };

export function Inspector({ task: t, tab, onTab, onClose }: Props) {
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
      remember(last);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  return (
    <aside className="inspector" style={{ width }} aria-label="Machine details">
      <div
        className="inspector-edge"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize the details"
        aria-valuemin={MIN}
        aria-valuemax={MAX}
        aria-valuenow={width}
        tabIndex={0}
        onPointerDown={drag}
        onKeyDown={(e) => {
          if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
          e.preventDefault();
          const next = Math.max(MIN, Math.min(MAX, width + (e.key === "ArrowLeft" ? 24 : -24)));
          setWidth(next);
          remember(next);
        }}
        title="Drag, or use the arrow keys, to resize"
      />
      <Tabs.Root value={tab} onValueChange={(v) => onTab(v as InspectorTab)} className="inspector-tabs">
        <Tabs.List className="tabs" aria-label="Details">
          <Tabs.Trigger value="telemetry">Telemetry</Tabs.Trigger>
          <Tabs.Trigger value="claude">Claude</Tabs.Trigger>
          <Tabs.Trigger value="diagnostics">Diagnostics</Tabs.Trigger>
          {/* On narrow screens the panel covers the toolbar button that opened it. */}
          <IconButton className="inspector-close" aria-label="Close the details" onClick={onClose}>
            <Icon name="close" />
          </IconButton>
        </Tabs.List>
        <Tabs.Content value="telemetry" className="tab-body">
          <TelemetryTab task={t} />
        </Tabs.Content>
        <Tabs.Content value="claude" className="tab-body">
          <ClaudeTab id={t.id} live={live} interactive={t.interactive} />
        </Tabs.Content>
        <Tabs.Content value="diagnostics" className="tab-body">
          <DiagnosticsTab id={t.id} ended={!live} />
        </Tabs.Content>
      </Tabs.Root>
    </aside>
  );
}
