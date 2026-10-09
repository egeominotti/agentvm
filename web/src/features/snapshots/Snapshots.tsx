// Saved copies of whole VMs: what is on this Mac and what is in S3. Restoring starts a new VM
// exactly from that point, with Claude continuing its conversation.
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { api } from "../../api/client";
import type { RemoteBackup } from "../../api/generated/RemoteBackup";
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";
import type { StorageUsage } from "../../api/generated/StorageUsage";
import { Button } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { useToast } from "../../components/Toast";
import { gb } from "../../lib/format";
import { LocalSnapshots } from "./LocalSnapshots";
import { RemoteBackups } from "./RemoteBackups";

type Tab = "local" | "s3";

export function Snapshots() {
  // Followed while a snapshot is being compressed: its real size shows when it is done.
  const q = useQuery({
    queryKey: ["snapshots"],
    queryFn: () => api<SnapshotMeta[]>("/api/snapshots"),
    refetchInterval: (query) => (query.state.data?.some((s) => s.compacting) ? 2000 : false),
  });
  const compacting = q.data?.some((s) => s.compacting) ?? false;
  const storage = useQuery({
    queryKey: ["storage", compacting],
    queryFn: () => api<StorageUsage>("/api/storage"),
    refetchInterval: compacting ? 2000 : false,
  });
  const backups = useQuery({ queryKey: ["backups"], queryFn: () => api<RemoteBackup[]>("/api/backups"), retry: false });
  const [tab, setTab] = useState<Tab>("local");
  const file = useRef<HTMLInputElement>(null);
  const qc = useQueryClient();
  const say = useToast();

  const importFile = async (f: File | undefined) => {
    if (!f) return;
    say(`Importing ${f.name}…`);
    try {
      const res = await fetch("/api/snapshots/import", {
        method: "POST",
        body: f,
        headers: { "content-type": "application/octet-stream" },
      });
      const data = (await res.json().catch(() => ({}))) as { name?: string; error?: string };
      if (res.ok) say(`Imported ${data.name ?? f.name}`);
      else say(data.error ?? "Import failed", "err");
    } catch {
      say("Import failed: the server is not reachable", "err");
    }
    qc.invalidateQueries({ queryKey: ["snapshots"] });
  };

  const list = q.data ?? [];
  const machines = new Set(list.map((s) => s.source_task)).size;
  const s3Ready = backups.isSuccess;
  return (
    <div className="wall-view snaps-view">
      <header className="view-head">
        <h1>Snapshots</h1>
        <span className="sub">Whole VMs saved at a moment: files, installed packages, Claude's conversation.</span>
        <span className="tb-gap" />
        <Button size="sm" onClick={() => file.current?.click()}>
          <Icon name="upload" />
          Import
        </Button>
        <input ref={file} type="file" accept=".zst,.tar,.gz" hidden onChange={(e) => importFile(e.target.files?.[0])} />
      </header>
      <div className="snaps-body">
        <dl className="snap-stats">
          <div>
            <dt>Snapshots</dt>
            <dd>{q.isSuccess ? list.length : "–"}</dd>
          </div>
          <div>
            <dt>On disk</dt>
            <dd>
              {storage.data ? gb(storage.data.snapshots_mb) : "–"}
              <small>{compacting ? "compressing…" : "compressed, shared"}</small>
            </dd>
          </div>
          <div>
            <dt>Machines</dt>
            <dd>{q.isSuccess ? machines : "–"}</dd>
          </div>
          <div>
            <dt>S3 backups</dt>
            <dd>
              {s3Ready ? backups.data.length : <a href="#/settings/backups">Set up</a>}
              {s3Ready ? <small>in the bucket</small> : null}
            </dd>
          </div>
        </dl>
        <div className="snap-tabs" role="tablist" aria-label="Where">
          {(
            [
              ["local", "On this Mac", list.length],
              ["s3", "In S3", s3Ready ? backups.data.length : null],
            ] as const
          ).map(([id, label, count]) => (
            <button key={id} type="button" role="tab" aria-selected={tab === id} onClick={() => setTab(id)}>
              {label}
              {count !== null ? <span className="count">{count}</span> : null}
            </button>
          ))}
        </div>
        {tab === "local" ? (
          <LocalSnapshots list={list} loaded={q.isSuccess} error={q.error?.message} s3Ready={s3Ready} />
        ) : (
          <RemoteBackups q={backups} />
        )}
      </div>
    </div>
  );
}
