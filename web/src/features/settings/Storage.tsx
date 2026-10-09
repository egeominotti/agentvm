// What agentvm keeps on this Mac's disk, and cleaning up after closed machines.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { StorageUsage } from "../../api/generated/StorageUsage";
import { ConfirmButton } from "../../components/ConfirmButton";
import { gb, plural } from "../../lib/format";
import { Fact } from "./Image";

export function Storage() {
  const qc = useQueryClient();
  const usage = useQuery({ queryKey: ["storage"], queryFn: () => api<StorageUsage>("/api/storage") });
  const cleanup = useMutation({
    mutationFn: () => api<{ removed: number }>("/api/storage/cleanup", "POST"),
    onSettled: () => qc.invalidateQueries({ queryKey: ["storage"] }),
  });
  const d = usage.data;
  return (
    <>
      <p className="lede">
        A closed machine's disk is deleted when it closes; one that was interrupted is kept as a snapshot. Logs and
        Claude's history stay until you delete them.
      </p>
      {d ? (
        <dl className="facts-grid">
          <Fact k="Running VM disks" v={gb(d.vm_disks_mb)} />
          <Fact k="Snapshots, before sharing" v={gb(d.snapshots_mb)} />
          <Fact k="VM image" v={gb(d.golden_mb)} />
          <Fact k={`Logs (${plural(d.jobs, "machine")})`} v={gb(d.jobs_mb)} />
        </dl>
      ) : null}
      <div className="row-actions">
        <ConfirmButton
          size="md"
          disabled={cleanup.isPending}
          confirm="Delete their logs and Claude's history for good?"
          onConfirm={() => cleanup.mutate()}
        >
          Delete logs of closed machines
        </ConfirmButton>
        {cleanup.data ? <span className="msg ok">Deleted {plural(cleanup.data.removed, "folder")}.</span> : null}
        {cleanup.error ? <span className="msg err">{cleanup.error.message}</span> : null}
      </div>
      <p className="hint">Their diagnostics and Claude's history go with them.</p>
    </>
  );
}
