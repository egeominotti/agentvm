// Machines that ended, as a table: how each ended, on which branch, for how long and at what
// cost; found by search or outcome, deleted one by one or several at once (branches stay).
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import type { TaskDto } from "../../api/generated/TaskDto";
import { keys } from "../../api/queries";
import { go } from "../../app/router";
import { Button, IconButton } from "../../components/Button";
import { ConfirmButton } from "../../components/ConfirmButton";
import { Icon } from "../../components/Icon";
import { Menu, MenuItem } from "../../components/Menu";
import { Segmented } from "../../components/Segmented";
import { StatusMark } from "../../components/StatusMark";
import { useToast } from "../../components/Toast";
import { ago, money, plural, repoName } from "../../lib/format";
import { age, shortId, titleOf } from "../../lib/task";
import { type EndedKind, filterEnded } from "./filter";

const KINDS: [EndedKind | "all", string][] = [
  ["all", "All"],
  ["done", "Done"],
  ["no_changes", "No changes"],
  ["failed", "Failed"],
  ["stopped", "Stopped"],
];

export function FinishedTable({ ended }: { ended: TaskDto[] }) {
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<EndedKind | "all">("all");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const qc = useQueryClient();
  const say = useToast();
  const newest = ended.toSorted((a, b) => (b.finished_at ?? 0) - (a.finished_at ?? 0));
  const shown = filterEnded(newest, query, kind);
  const selected = shown.filter((t) => picked.has(t.id));
  const remove = useMutation({
    // One after the other, as they were when you clicked: the list refreshes every second.
    mutationFn: async (ids: string[]) => {
      for (const id of ids) await api(`/api/tasks/${id}`, "DELETE");
      return ids.length;
    },
    onSuccess: (n) => {
      say(`Deleted ${plural(n, "finished machine")}. Their branches stay in your repository.`);
      setPicked(new Set());
    },
    onError: (e: Error) => say(e.message, "err"),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.tasks }),
  });
  const toggle = (id: string, on: boolean) =>
    setPicked((now) => {
      const next = new Set(now);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });

  if (!ended.length) {
    return (
      <div className="empty-panel">
        <Icon name="machines" />
        <b>No finished machines</b>
        <p>Machines you close or that finish their task show here, with the branch their work went to.</p>
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
            aria-label="Search finished machines"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <Segmented label="Outcome" value={kind} options={KINDS} onChange={setKind} />
      </div>
      {selected.length ? (
        <div className="bulk-bar" role="toolbar" aria-label="Selected machines">
          <b>{plural(selected.length, "machine")} selected</b>
          <Button size="sm" variant="ghost" onClick={() => setPicked(new Set())}>
            Clear
          </Button>
          <span className="tb-gap" />
          <ConfirmButton
            confirm={`Delete ${plural(selected.length, "machine")} with their logs? Branches stay`}
            disabled={remove.isPending}
            onConfirm={() => remove.mutate(selected.map((t) => t.id))}
          >
            Delete
          </ConfirmButton>
        </div>
      ) : null}
      <div className="data-table cols-finished" role="table" aria-label="Finished machines">
        <div className="dt-tr dt-th" role="row">
          <span role="columnheader">
            <input
              type="checkbox"
              aria-label="Select every machine shown"
              checked={!!shown.length && selected.length === shown.length}
              onChange={(e) => setPicked(e.target.checked ? new Set(shown.map((t) => t.id)) : new Set())}
            />
          </span>
          <span role="columnheader">Outcome</span>
          <span role="columnheader">Machine</span>
          <span role="columnheader" className="wide-only">
            Branch
          </span>
          <span role="columnheader" className="num wide-only">
            Ran
          </span>
          <span role="columnheader" className="num wide-only">
            Claude
          </span>
          <span role="columnheader" className="num wide-only">
            Ended
          </span>
          <span role="columnheader" aria-label="Actions" />
        </div>
        {shown.map((t) => (
          <div key={t.id} className={`dt-tr dt-row${picked.has(t.id) ? " selected" : ""}`} role="row">
            <span role="cell">
              <input
                type="checkbox"
                aria-label={`Select ${titleOf(t)}`}
                checked={picked.has(t.id)}
                onChange={(e) => toggle(t.id, e.target.checked)}
              />
            </span>
            <span role="cell">
              <StatusMark task={t} label />
            </span>
            <span role="cell" className="dt-name">
              <a href={`#/vm/${t.id}`} title={`Open: ${titleOf(t)}`}>
                <b>{titleOf(t)}</b>
              </a>
              <span className="muted">
                {repoName(t.repo)} · #{shortId(t)}
              </span>
            </span>
            <span role="cell" className="muted mono wide-only" title={t.branch}>
              {t.branch}
            </span>
            <span role="cell" className="num muted wide-only">
              {age(t)}
            </span>
            <span role="cell" className="num muted wide-only">
              {t.usage?.cost_usd ? money(t.usage.cost_usd) : "–"}
            </span>
            <span
              role="cell"
              className="num muted wide-only"
              title={t.finished_at ? new Date(t.finished_at * 1000).toLocaleString() : ""}
            >
              {t.finished_at ? ago(t.finished_at) : "–"}
            </span>
            <span role="cell" className="dt-actions">
              <Button size="sm" onClick={() => go(`#/vm/${t.id}`)}>
                Open
              </Button>
              <Menu
                trigger={
                  <IconButton aria-label={`More for ${titleOf(t)}`}>
                    <Icon name="more" />
                  </IconButton>
                }
              >
                <MenuItem
                  icon="trash"
                  danger
                  confirm="Delete it with its logs? Its branch stays"
                  onSelect={() => remove.mutate([t.id])}
                >
                  Delete
                </MenuItem>
              </Menu>
            </span>
          </div>
        ))}
        {!shown.length ? <p className="dt-none">No finished machine matches.</p> : null}
      </div>
    </>
  );
}
