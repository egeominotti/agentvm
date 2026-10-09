// Snapshots backed up to S3, brought back to this Mac on demand.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { RemoteBackup } from "../../api/generated/RemoteBackup";
import { Button, IconButton } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Menu, MenuItem } from "../../components/Menu";
import { useToast } from "../../components/Toast";
import { gb, repoName } from "../../lib/format";

export function Backups() {
  const q = useQuery({ queryKey: ["backups"], queryFn: () => api<RemoteBackup[]>("/api/backups"), retry: false });
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
      say("Now in your snapshots");
      refresh();
    },
    onError: (e: Error) => say(e.message, "err"),
  });
  const remove = useMutation({
    mutationFn: (id: string) => api(`/api/backups/${id}`, "DELETE"),
    onSuccess: refresh,
    onError: (e: Error) => say(e.message, "err"),
  });
  const notSet = q.error && /not configured/i.test(q.error.message);
  return (
    <section className="snap-group" aria-label="Backups in S3">
      <header>
        <h2>In S3</h2>
      </header>
      {q.error ? (
        <p className="hint">
          {notSet ? (
            <>
              Keep copies off this Mac: <a href="#/settings/s3">set up a bucket in Settings</a>.
            </>
          ) : (
            q.error.message
          )}
        </p>
      ) : !q.data ? (
        <p className="hint">Reading the bucket…</p>
      ) : !q.data.length ? (
        <p className="hint">No backups in the bucket yet. Use “Back up to S3” in a snapshot's menu.</p>
      ) : (
        <ul className="snaps">
          {q.data.map((b) => (
            <li key={b.snapshot.id} className="snap">
              <div className="snap-main">
                <b>{b.snapshot.name}</b>
                <span className="snap-meta">
                  {repoName(b.snapshot.repo)} · {new Date(b.snapshot.created_at * 1000).toLocaleString()} ·{" "}
                  {gb(b.archive_mb)} compressed
                </span>
              </div>
              <Menu
                trigger={
                  <IconButton aria-label="More">
                    <Icon name="more" />
                  </IconButton>
                }
              >
                <MenuItem
                  icon="trash"
                  danger
                  confirm="Delete it from S3?"
                  onSelect={() => remove.mutate(b.snapshot.id)}
                >
                  Delete from S3
                </MenuItem>
              </Menu>
              <Button disabled={b.local || bring.isPending} onClick={() => bring.mutate(b.snapshot.id)}>
                <Icon name={b.local ? "check" : "cloud-down"} />
                {b.local ? "On this Mac" : "Bring to this Mac"}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
