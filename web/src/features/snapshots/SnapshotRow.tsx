// One snapshot in the table: what it is, when, what it costs, and what can be done with it.
// Restoring (or resuming an interrupted machine) is the main thing; the rest waits in "⋯".
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";
import { keys } from "../../api/queries";
import { go } from "../../app/router";
import { Button, IconButton } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Menu, MenuItem } from "../../components/Menu";
import { useToast } from "../../components/Toast";
import { ago, gb } from "../../lib/format";
import { modelLabel } from "../../lib/models";
import type { Kind } from "./filter";

const KIND: Record<Kind, string> = { manual: "Manual", auto: "Automatic", interrupted: "Interrupted" };

type Props = {
  s: SnapshotMeta;
  label: string;
  kind: Kind;
  selected: boolean;
  onSelect: (on: boolean) => void;
  s3Ready: boolean;
};

export function SnapshotRow({ s, label, kind, selected, onSelect, s3Ready }: Props) {
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
      say(`Backed up to S3 (${gb(r.archive_mb)})`);
      qc.invalidateQueries({ queryKey: ["backups"] });
    },
    onError: fail,
  });
  const resume = kind === "interrupted";
  const taken = new Date(s.created_at * 1000);
  return (
    <div className={`dt-tr dt-row${selected ? " selected" : ""}${resume ? " interrupted" : ""}`} role="row">
      <span role="cell">
        <input
          type="checkbox"
          aria-label={`Select ${label}`}
          checked={selected}
          onChange={(e) => onSelect(e.target.checked)}
        />
      </span>
      <span role="cell" className="dt-name">
        <b title={s.name}>{label}</b>
        <span className="muted">
          {modelLabel(s.model)}
          {s.cpus ? ` · ${s.cpus} vCPUs, ${gb(s.memory_mb)}` : ""}
        </span>
      </span>
      <span role="cell">
        <span className={`kind ${kind}`}>{KIND[kind]}</span>
      </span>
      <span role="cell" className="muted" title={taken.toLocaleString()}>
        {ago(s.created_at)}
      </span>
      <span role="cell" className="num" title="What deleting it frees: the compressed data only this snapshot holds">
        {s.compacting ? <span className="muted">compressing…</span> : s.size_mb < 1 ? "< 1 MB" : gb(s.size_mb)}
      </span>
      <span role="cell" className="dt-actions">
        <Button
          size="sm"
          variant={resume ? "primary" : "default"}
          disabled={restore.isPending}
          onClick={() => restore.mutate()}
          title="Starts a new VM exactly from this point; Claude continues its conversation"
        >
          <Icon name="restore" />
          {restore.isPending ? "Starting…" : resume ? "Resume" : "Restore"}
        </Button>
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
          <MenuItem icon="cloud-up" disabled={!s3Ready || backup.isPending} onSelect={() => backup.mutate()}>
            {s3Ready ? "Back up to S3" : "Back up to S3 (set up S3 first)"}
          </MenuItem>
          <MenuItem icon="trash" danger confirm="Delete it for good?" onSelect={() => remove.mutate()}>
            Delete
          </MenuItem>
        </Menu>
      </span>
    </div>
  );
}
