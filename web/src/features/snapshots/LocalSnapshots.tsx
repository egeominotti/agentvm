// The snapshots on this Mac as a table: found by name or repository, filtered by kind, grouped by
// the machine they came from; several at once can be backed up or deleted.
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";
import { Button } from "../../components/Button";
import { ConfirmButton } from "../../components/ConfirmButton";
import { Icon } from "../../components/Icon";
import { Segmented } from "../../components/Segmented";
import { useToast } from "../../components/Toast";
import { plural, repoName } from "../../lib/format";
import { filterSnapshots, type KindFilter, kindOf } from "./filter";
import { groupSnapshots, labelOf } from "./groups";
import { SnapshotRow } from "./SnapshotRow";

const KINDS: [KindFilter, string][] = [
  ["all", "All"],
  ["manual", "Manual"],
  ["auto", "Automatic"],
  ["interrupted", "Interrupted"],
];

type Props = { list: SnapshotMeta[]; loaded: boolean; error?: string; s3Ready: boolean };

export function LocalSnapshots({ list, loaded, error, s3Ready }: Props) {
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<KindFilter>("all");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const shown = filterSnapshots(list, query, kind);
  const groups = groupSnapshots(shown);
  const selected = shown.filter((s) => picked.has(s.id));
  const toggle = (id: string, on: boolean) =>
    setPicked((now) => {
      const next = new Set(now);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });
  const clear = () => setPicked(new Set());
  const remove = useBulk("Deleting", (id) => `/api/snapshots/${id}`, "DELETE", "deleted", clear);
  const backup = useBulk("Backing up", (id) => `/api/snapshots/${id}/backup`, "POST", "backed up to S3", clear);

  if (error) return <p className="msg err">{error}</p>;
  if (loaded && !list.length) {
    return (
      <div className="empty-panel">
        <Icon name="snapshots" />
        <b>No snapshots yet</b>
        <p>
          Take one from a running machine's ⋯ menu, or let them be taken on a schedule:{" "}
          <a href="#/settings/snapshots">Settings › Snapshots</a>.
        </p>
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
            placeholder="Search by name or repository"
            aria-label="Search snapshots"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <Segmented label="Kind" value={kind} options={KINDS} onChange={setKind} />
      </div>
      {selected.length ? (
        <div className="bulk-bar" role="toolbar" aria-label="Selected snapshots">
          <b>{plural(selected.length, "snapshot")} selected</b>
          <Button size="sm" variant="ghost" onClick={() => setPicked(new Set())}>
            Clear
          </Button>
          <span className="tb-gap" />
          {s3Ready ? (
            <Button size="sm" disabled={backup.isPending} onClick={() => backup.mutate(selected.map((s) => s.id))}>
              <Icon name="cloud-up" />
              Back up to S3
            </Button>
          ) : null}
          <ConfirmButton
            confirm={`Delete ${plural(selected.length, "snapshot")} for good?`}
            disabled={remove.isPending}
            onConfirm={() => remove.mutate(selected.map((s) => s.id))}
          >
            Delete
          </ConfirmButton>
        </div>
      ) : null}
      <div className="data-table cols-snapshots" role="table" aria-label="Snapshots on this Mac">
        <div className="dt-tr dt-th" role="row">
          <span role="columnheader">
            <input
              type="checkbox"
              aria-label="Select every snapshot shown"
              checked={!!shown.length && selected.length === shown.length}
              onChange={(e) => setPicked(e.target.checked ? new Set(shown.map((s) => s.id)) : new Set())}
            />
          </span>
          <span role="columnheader">Snapshot</span>
          <span role="columnheader">Kind</span>
          <span role="columnheader">Taken</span>
          <span role="columnheader" className="num">
            Size
          </span>
          <span role="columnheader" aria-label="Actions" />
        </div>
        {groups.map((g) => (
          <div key={g.task} role="rowgroup" className="dt-group">
            <div className="dt-tr dt-group-head" role="row">
              <span role="cell" />
              <span role="cell" title={`Restores into ${g.repo}, on a branch of its own`}>
                <b>{g.title}</b> <span className="muted">{repoName(g.repo)}</span>
              </span>
              <span role="cell" className="muted">
                {plural(g.interrupted.length + g.manual.length + g.auto.length, "snapshot")}
              </span>
            </div>
            {[...g.interrupted, ...g.manual, ...g.auto].map((s) => (
              <SnapshotRow
                key={s.id}
                s={s}
                label={labelOf(s, g.title)}
                kind={kindOf(s)}
                selected={picked.has(s.id)}
                onSelect={(on) => toggle(s.id, on)}
                s3Ready={s3Ready}
              />
            ))}
          </div>
        ))}
        {!shown.length && list.length ? <p className="dt-none">No snapshot matches.</p> : null}
      </div>
    </>
  );
}

/** The same request for several snapshots, one after the other: deleting or uploading many at
 *  once would compete for the same disk. */
function useBulk(
  verb: string,
  path: (id: string) => string,
  method: "POST" | "DELETE",
  done: string,
  after: () => void,
) {
  const qc = useQueryClient();
  const say = useToast();
  return useMutation({
    mutationFn: async (ids: string[]) => {
      for (const id of ids) await api(path(id), method);
      return ids.length;
    },
    onMutate: (ids) => say(`${verb} ${plural(ids.length, "snapshot")}…`),
    onSuccess: (n) => {
      say(`${plural(n, "snapshot")} ${done}`);
      after();
      qc.invalidateQueries({ queryKey: ["snapshots"] });
      qc.invalidateQueries({ queryKey: ["backups"] });
    },
    onError: (e: Error) => say(e.message, "err"),
  });
}
