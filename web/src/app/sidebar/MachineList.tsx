// The running machines (and the one open), each with its state and how long it has run.
import type { TaskDto } from "../../api/generated/TaskDto";
import { StatusMark } from "../../components/StatusMark";
import { age, isEnded, isWaiting, shortId, statusOf, titleOf } from "../../lib/task";

export function MachineList({ tasks, focused }: { tasks: TaskDto[]; focused: string | null }) {
  const shown = tasks.filter((t) => !isEnded(t) || t.id === focused);
  return (
    <section className="side-machines" aria-labelledby="side-machines-label">
      <h2 id="side-machines-label" className="side-label">
        Machines
      </h2>
      {shown.length === 0 ? <p className="side-empty">No machine running. Press ⌘K to start one.</p> : null}
      {shown.map((t) => (
        <a
          key={t.id}
          className={`side-vm${isWaiting(t) ? " waiting" : ""}`}
          href={`#/vm/${t.id}`}
          aria-current={t.id === focused ? "page" : undefined}
          title={`${titleOf(t)} · ${shortId(t)}\n${statusOf(t).label}`}
        >
          <StatusMark task={t} />
          <span className="t">{titleOf(t)}</span>
          <span className="m num">{isWaiting(t) ? "your turn" : age(t)}</span>
        </a>
      ))}
    </section>
  );
}
