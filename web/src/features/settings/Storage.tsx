// What agentvm keeps on this Mac's disk, and cleaning up after closed machines.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { StorageUsage } from "../../api/generated/StorageUsage";
import { ConfirmButton } from "../../components/ConfirmButton";
import { gb, plural } from "../../lib/format";
import { Page, Panel, Row } from "./kit";

export function Storage() {
  const qc = useQueryClient();
  const usage = useQuery({ queryKey: ["storage"], queryFn: () => api<StorageUsage>("/api/storage") });
  const cleanup = useMutation({
    mutationFn: () => api<{ removed: number }>("/api/storage/cleanup", "POST"),
    onSettled: () => qc.invalidateQueries({ queryKey: ["storage"] }),
  });
  const d = usage.data;
  const parts = d
    ? [
        { label: "Running VM disks", mb: d.vm_disks_mb, color: "var(--brand)" },
        { label: "Snapshots", mb: d.snapshots_mb, color: "var(--done)" },
        { label: "VM image", mb: d.golden_mb, color: "var(--work)" },
        { label: `Logs of ${plural(d.jobs, "machine")}`, mb: d.jobs_mb, color: "var(--ink-3)" },
      ]
    : [];
  const total = parts.reduce((a, p) => a + p.mb, 0);
  return (
    <Page
      title="Storage"
      description="A closed VM's disk is deleted as it closes; an interrupted one is kept as a snapshot. Logs and Claude's history stay until you delete them."
    >
      <Panel title="On this Mac" description={d ? `${gb(total)} in ~/AgentVMs.` : undefined}>
        {d ? (
          <>
            <Row label="Disk usage" wide>
              <div className="usage-bar" role="img" aria-label={parts.map((p) => `${p.label} ${gb(p.mb)}`).join(", ")}>
                {parts.map((p) =>
                  p.mb > 0 ? <i key={p.label} style={{ flexGrow: p.mb, background: p.color }} /> : null,
                )}
              </div>
            </Row>
            <ul className="set-list">
              {parts.map((p) => (
                <li key={p.label}>
                  <span className="swatch" style={{ background: p.color }} />
                  {p.label}
                  <span className="set-value">{gb(p.mb)}</span>
                </li>
              ))}
            </ul>
          </>
        ) : (
          <Row label="Disk usage" description="Measuring…" />
        )}
      </Panel>
      <Panel title="Danger zone" danger>
        <Row
          label="Delete logs of closed machines"
          description={
            cleanup.data
              ? `Deleted ${plural(cleanup.data.removed, "folder")}.`
              : "Their diagnostics and Claude's history go with them, for good."
          }
        >
          <ConfirmButton
            size="md"
            disabled={cleanup.isPending}
            confirm="Delete them for good?"
            onConfirm={() => cleanup.mutate()}
          >
            Delete logs
          </ConfirmButton>
        </Row>
      </Panel>
      {cleanup.error ? <p className="msg err">{cleanup.error.message}</p> : null}
    </Page>
  );
}
