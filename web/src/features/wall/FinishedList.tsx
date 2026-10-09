// Machines that ended: one line each, newest first, with what they produced.
import type { TaskDto } from "../../api/generated/TaskDto";
import { StatusMark } from "../../components/StatusMark";
import { ago, money, repoName } from "../../lib/format";
import { age, shortId, titleOf } from "../../lib/task";

export function FinishedList({ tasks }: { tasks: TaskDto[] }) {
  const newest = [...tasks].sort((a, b) => (b.finished_at ?? 0) - (a.finished_at ?? 0));
  return (
    <ul className="finished">
      {newest.map((t) => (
        <li key={t.id}>
          <a href={`#/vm/${t.id}`}>
            <StatusMark task={t} label />
            <span className="f-title">{titleOf(t)}</span>
            <span className="f-id">#{shortId(t)}</span>
            <span className="f-repo">{repoName(t.repo)}</span>
            <span className="f-cost">{t.usage?.cost_usd ? money(t.usage.cost_usd) : ""}</span>
            <span className="f-age" title={`Ran for ${age(t)}`}>
              {t.finished_at ? ago(t.finished_at) : age(t)}
            </span>
          </a>
        </li>
      ))}
    </ul>
  );
}
