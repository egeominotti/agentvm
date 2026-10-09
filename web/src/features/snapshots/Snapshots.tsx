// Saved copies of whole VMs, by the machine they came from. Restoring starts a new VM exactly
// from that point, with Claude continuing its conversation.
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef } from "react";
import { api } from "../../api/client";
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";
import { Button } from "../../components/Button";
import { useToast } from "../../components/Toast";
import { plural, repoName } from "../../lib/format";
import { Backups } from "./Backups";
import { groupSnapshots, labelOf } from "./groups";
import { SnapshotRow } from "./SnapshotRow";

export function Snapshots() {
  const q = useQuery({ queryKey: ["snapshots"], queryFn: () => api<SnapshotMeta[]>("/api/snapshots") });
  const groups = groupSnapshots(q.data ?? []);
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

  return (
    <div className="wall-view">
      <header className="view-head">
        <h1>Snapshots</h1>
        <span className="sub">Whole VMs saved at a moment: files, installed packages, Claude's conversation.</span>
        <span className="tb-gap" />
        <Button size="sm" onClick={() => file.current?.click()}>
          Import from file
        </Button>
        <input ref={file} type="file" accept=".zst,.tar,.gz" hidden onChange={(e) => importFile(e.target.files?.[0])} />
      </header>
      <div className="wall-scroll snapshots">
        {q.error ? <p className="hint">{q.error.message}</p> : null}
        {q.isSuccess && !groups.length ? (
          <div className="empty-card">
            <b>No snapshots yet</b>
            <p className="hint">
              Open a running machine and choose “Take a snapshot now” in its ⋯ menu, or let automatic snapshots save it
              on a schedule (Settings › Automatic snapshots).
            </p>
          </div>
        ) : null}
        {groups.map((g) => (
          <section key={g.task} className="snap-group" aria-label={g.title}>
            <header>
              <h2>{g.title}</h2>
              <span className="sub">{repoName(g.repo)}</span>
            </header>
            <ul className="snaps">
              {g.interrupted.map((s) => (
                <SnapshotRow key={s.id} s={s} label="Interrupted: its disk was kept" resume />
              ))}
              {g.manual.map((s) => (
                <SnapshotRow key={s.id} s={s} label={labelOf(s, g.title)} />
              ))}
            </ul>
            {g.auto.length ? (
              <details className="auto-snaps">
                <summary>{plural(g.auto.length, "automatic snapshot")}</summary>
                <ul className="snaps">
                  {g.auto.map((s) => (
                    <SnapshotRow key={s.id} s={s} label={labelOf(s, g.title)} />
                  ))}
                </ul>
              </details>
            ) : null}
          </section>
        ))}
        {groups.length ? (
          <p className="hint">
            Snapshots share their unchanged blocks with the VM image and with each other, so on disk they take far less
            than their sizes add up to.
          </p>
        ) : null}
        <Backups />
      </div>
    </div>
  );
}
