// Every machine at once: the running ones live, the finished ones in a list below.
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import { keys, useTasks } from "../../api/queries";
import { Button } from "../../components/Button";
import { useToast } from "../../components/Toast";
import { plural } from "../../lib/format";
import { isEnded, isWaiting } from "../../lib/task";
import { openLauncher } from "../launcher/open";
import { EmptyState } from "./EmptyState";
import { FinishedList } from "./FinishedList";
import { MachineCard } from "./MachineCard";

export function Wall() {
  const q = useTasks();
  const qc = useQueryClient();
  const say = useToast();
  const tasks = q.data ?? [];
  const live = tasks.filter((t) => !isEnded(t));
  const ended = tasks.filter(isEnded);
  const waiting = live.filter(isWaiting).length;
  const clear = useMutation({
    mutationFn: async () => {
      for (const t of ended) await api(`/api/tasks/${t.id}`, "DELETE");
    },
    onSuccess: () => say(`Removed ${plural(ended.length, "finished machine")} from the list. Their branches stay.`),
    onError: (e: Error) => say(e.message, "err"),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.tasks }),
  });

  if (q.error && !q.data) {
    return (
      <div className="empty-state">
        <b>Cannot reach agentvm</b>
        <span>{q.error.message} Retrying…</span>
      </div>
    );
  }
  if (q.isSuccess && !tasks.length) return <EmptyState />;

  return (
    <div className="wall-view">
      <header className="view-head">
        <h1>Machines</h1>
        <span className="sub">
          {live.length ? plural(live.length, "running machine") : "Nothing running"}
          {waiting ? `, ${waiting} waiting for you` : ""}
        </span>
        <span className="tb-gap" />
        <Button variant="primary" onClick={openLauncher}>
          New VM <kbd>⌘K</kbd>
        </Button>
      </header>
      <div className="wall-scroll">
        {live.length ? (
          <div className="cards">
            {live.map((t) => (
              <MachineCard key={t.id} task={t} />
            ))}
          </div>
        ) : null}
        {ended.length ? (
          <section className="finished-section" aria-label="Finished machines">
            <header>
              <h2>Finished</h2>
              <Button size="sm" variant="ghost" disabled={clear.isPending} onClick={() => clear.mutate()}>
                Clear {ended.length} from the list
              </Button>
            </header>
            <FinishedList tasks={ended} />
          </section>
        ) : null}
      </div>
    </div>
  );
}
