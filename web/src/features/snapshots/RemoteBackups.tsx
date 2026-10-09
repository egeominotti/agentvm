// Snapshots backed up to S3, as a table: brought back to this Mac on demand, or deleted there.
import { type UseQueryResult, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { RemoteBackup } from "../../api/generated/RemoteBackup";
import { Button, IconButton } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Menu, MenuItem } from "../../components/Menu";
import { useToast } from "../../components/Toast";
import { ago, gb, repoName } from "../../lib/format";

export function RemoteBackups({ q }: { q: UseQueryResult<RemoteBackup[], Error> }) {
  const qc = useQueryClient();
  const say = useToast();
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["backups"] });
    qc.invalidateQueries({ queryKey: ["snapshots"] });
  };
  const bring = useMutation({
    mutationFn: (id: string) => api(`/api/backups/${id}/restore`, "POST"),
    onMutate: () => say("Downloading from S3…"),
    onSuccess: () => {
      say("Now on this Mac");
      refresh();
    },
    onError: (e: Error) => say(e.message, "err"),
  });
  const remove = useMutation({
    mutationFn: (id: string) => api(`/api/backups/${id}`, "DELETE"),
    onSuccess: refresh,
    onError: (e: Error) => say(e.message, "err"),
  });
  if (q.error) {
    const notSet = /not configured/i.test(q.error.message);
    return (
      <div className="snap-empty">
        <Icon name="cloud-up" />
        <b>{notSet ? "Keep copies off this Mac" : "The bucket cannot be read"}</b>
        <p>{notSet ? <a href="#/settings/backups">Set up an S3 bucket in Settings › Backups</a> : q.error.message}</p>
      </div>
    );
  }
  if (!q.data) return <p className="msg">Reading the bucket…</p>;
  if (!q.data.length) {
    return (
      <div className="snap-empty">
        <Icon name="cloud-up" />
        <b>No backups yet</b>
        <p>Choose “Back up to S3” in a snapshot's ⋯ menu, or select several and back them up at once.</p>
      </div>
    );
  }
  return (
    <div className="snap-table remote" role="table" aria-label="Backups in S3">
      <div className="snap-tr snap-th" role="row">
        <span role="columnheader">Snapshot</span>
        <span role="columnheader">Repository</span>
        <span role="columnheader">Taken</span>
        <span role="columnheader" className="num">
          Size
        </span>
        <span role="columnheader" aria-label="Actions" />
      </div>
      {q.data.map((b) => (
        <div key={b.snapshot.id} className="snap-tr snap-row" role="row">
          <span role="cell" className="snap-name">
            <b title={b.snapshot.name}>{b.snapshot.name}</b>
            <span className="muted">Uploaded {b.uploaded_at}</span>
          </span>
          <span role="cell" className="muted" title={b.snapshot.repo}>
            {repoName(b.snapshot.repo)}
          </span>
          <span role="cell" className="muted" title={new Date(b.snapshot.created_at * 1000).toLocaleString()}>
            {ago(b.snapshot.created_at)}
          </span>
          <span role="cell" className="num">
            {gb(b.archive_mb)}
          </span>
          <span role="cell" className="snap-actions">
            <Button size="sm" disabled={b.local || bring.isPending} onClick={() => bring.mutate(b.snapshot.id)}>
              <Icon name={b.local ? "check" : "cloud-down"} />
              {b.local ? "On this Mac" : "Bring here"}
            </Button>
            <Menu
              trigger={
                <IconButton aria-label={`More for ${b.snapshot.name}`}>
                  <Icon name="more" />
                </IconButton>
              }
            >
              <MenuItem icon="trash" danger confirm="Delete it from S3?" onSelect={() => remove.mutate(b.snapshot.id)}>
                Delete from S3
              </MenuItem>
            </Menu>
          </span>
        </div>
      ))}
    </div>
  );
}
