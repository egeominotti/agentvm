// One snapshot: what it is, when, and what can be done with it. Restoring is the main thing;
// downloading, backing up and deleting wait in the "⋯" menu.
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";
import { keys } from "../../api/queries";
import { go } from "../../app/router";
import { Button, IconButton } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Menu, MenuItem } from "../../components/Menu";
import { useToast } from "../../components/Toast";
import { gb } from "../../lib/format";
import { modelLabel } from "../../lib/models";

const when = (at: number) =>
  new Date(at * 1000).toLocaleString([], { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

export function SnapshotRow({ s, label, resume }: { s: SnapshotMeta; label: string; resume?: boolean }) {
  const qc = useQueryClient();
  const say = useToast();
  const fail = (e: Error) => say(e.message, "err");
  const restore = useMutation({
    mutationFn: () => api<{ id: string }>(`/api/snapshots/${s.id}/restore`, "POST"),
    onSuccess: async (r) => {
      await qc.invalidateQueries({ queryKey: keys.tasks });
      go(`#/vm/${r.id}`);
    },
    onError: fail,
  });
  const remove = useMutation({
    mutationFn: () => api(`/api/snapshots/${s.id}`, "DELETE"),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["snapshots"] }),
    onError: fail,
  });
  const backup = useMutation({
    mutationFn: () => api<{ archive_mb: number }>(`/api/snapshots/${s.id}/backup`, "POST"),
    onMutate: () => say("Uploading to S3…"),
    onSuccess: (r) => {
      say(`Backed up to S3 (${gb(r.archive_mb)} compressed)`);
      qc.invalidateQueries({ queryKey: ["backups"] });
    },
    onError: fail,
  });
  return (
    <li className={`snap${resume ? " interrupted" : ""}`}>
      <div className="snap-main">
        <b>{label}</b>
        <span className="snap-meta">
          {when(s.created_at)} · {s.cpus ? `${s.cpus} vCPUs, ${gb(s.memory_mb)} · ` : ""}
          {modelLabel(s.model)} ·{" "}
          {s.compacting ? (
            <span title="Being compressed into the chunks snapshots share">compressing…</span>
          ) : (
            <span title="What deleting it frees: the compressed data only this snapshot holds">
              {s.size_mb < 1 ? "< 1 MB" : gb(s.size_mb)}
            </span>
          )}
        </span>
      </div>
      <Menu
        trigger={
          <IconButton aria-label={`More for ${label}`}>
            <Icon name="more" />
          </IconButton>
        }
      >
        <MenuItem icon="download" onSelect={() => window.open(`/api/snapshots/${s.id}/export`, "_blank")}>
          Download (.tar.zst)
        </MenuItem>
        <MenuItem icon="cloud-up" disabled={backup.isPending} onSelect={() => backup.mutate()}>
          Back up to S3
        </MenuItem>
        <MenuItem icon="trash" danger confirm="Delete it for good?" onSelect={() => remove.mutate()}>
          Delete
        </MenuItem>
      </Menu>
      <Button
        variant={resume ? "primary" : "default"}
        disabled={restore.isPending}
        onClick={() => restore.mutate()}
        title="Starts a new VM exactly from this point; Claude continues its conversation"
      >
        <Icon name="restore" />
        {restore.isPending ? "Starting…" : resume ? "Resume" : "Restore"}
      </Button>
    </li>
  );
}
