// A machine's state at a glance: the same colored mark, and its words where there is room.
import type { TaskDto } from "../api/generated/TaskDto";
import { statusOf } from "../lib/task";

export function StatusMark({ task, label = false }: { task: TaskDto; label?: boolean }) {
  const { tone, label: text } = statusOf(task);
  return (
    <span className={`status tone-${tone}`} title={label ? undefined : text}>
      <span className="dot" aria-hidden="true" />
      {label ? <span className="status-label">{text}</span> : <span className="sr-only">{text}</span>}
    </span>
  );
}
