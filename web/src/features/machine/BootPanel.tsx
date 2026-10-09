// The boot of a machine, until its terminal is ready: the real steps, with their timings.
import type { TaskDto } from "../../api/generated/TaskDto";
import { age } from "../../lib/task";
import { bootSteps } from "./boot";

export function BootPanel({ task: t }: { task: TaskDto }) {
  const { steps, current } = bootSteps(t);
  const now = current < steps.length ? `${steps[current]?.label}…` : "Opening the terminal…";
  return (
    <div className="boot" role="status" aria-live="polite">
      <BootMark />
      <div className="boot-head">
        <span className="boot-kicker">{t.status.state === "queued" ? "Queued" : "Starting your machine"}</span>
        <b className="boot-now">{now}</b>
        <span className="boot-clock">{age(t)}</span>
      </div>
      <div className="boot-bar">
        <i style={{ width: `${(100 * current) / steps.length}%` }} />
      </div>
      <ol className="boot-steps">
        {steps.map((s, k) => (
          <li key={s.label} className={s.failed ? "failed" : s.done ? "done" : k === current ? "now" : ""}>
            <span className="tick" />
            <span className="what">{s.label}</span>
            <span className="when">{s.detail}</span>
          </li>
        ))}
      </ol>
    </div>
  );
}

export const BootMark = () => (
  <svg viewBox="0 0 64 64" className="boot-mark" aria-hidden="true">
    <g className="brackets" strokeLinecap="square" fill="none">
      <path d="M6 20V6h14" />
      <path d="M44 6h14v14" />
      <path d="M58 44v14H44" />
      <path d="M20 58H6V44" />
    </g>
    <rect className="core" x="22" y="22" width="12" height="20" />
    <rect className="scan" x="8" y="8" width="48" height="2" />
  </svg>
);
