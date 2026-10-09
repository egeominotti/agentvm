// The Debian image every VM starts from: its state, the Claude Code it will have, its rebuild.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { GoldenStatus } from "../../api/generated/GoldenStatus";
import type { Settings } from "../../api/generated/Settings";
import { useReleases } from "../../api/queries";
import { Button } from "../../components/Button";
import { gb } from "../../lib/format";
import type { SetSetting } from "./useDraft";

export function Image({ s, set }: { s: Settings; set: SetSetting }) {
  const qc = useQueryClient();
  // Read again every 1.5 s while a rebuild runs, to show its log as it goes.
  const golden = useQuery({
    queryKey: ["golden"],
    queryFn: () => api<GoldenStatus>("/api/golden"),
    refetchInterval: (q) => (q.state.data?.rebuilding ? 1500 : false),
  });
  const releases = useReleases().data;
  const rebuild = useMutation({
    mutationFn: () => api("/api/golden/rebuild", "POST"),
    onSettled: () => qc.invalidateQueries({ queryKey: ["golden"] }),
  });
  const d = golden.data;
  return (
    <>
      <p className="lede">
        Every VM starts as an instant copy of this Debian 13 image with Claude Code installed. Rebuild it to update
        Claude Code and the system packages; running VMs keep theirs.
      </p>
      {d ? (
        <dl className="facts-grid">
          <Fact k="State" v={d.rebuilding ? "Rebuilding…" : d.exists ? "Ready" : "Missing"} />
          <Fact k="Claude Code" v={d.claude_version ?? "unknown"} />
          <Fact k="Built" v={d.built_at ? new Date(d.built_at * 1000).toLocaleString() : "never"} />
          <Fact k="Size on disk" v={gb(d.size_mb)} />
        </dl>
      ) : null}
      <label className="set narrow">
        <span className="set-label">Claude Code for the next rebuild</span>
        <select className="text-input" value={s.claude_version} onChange={(e) => set("claude_version", e.target.value)}>
          <option value="latest">{releases ? `Latest (${releases.latest})` : "Latest"}</option>
          <option value="stable">{releases ? `Stable (${releases.stable})` : "Stable"}</option>
          {releases?.versions.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
          {releases || ["latest", "stable"].includes(s.claude_version) ? null : (
            <option value={s.claude_version}>{s.claude_version}</option>
          )}
        </select>
        <span className="hint">A launch can still pick another version. Claude Code never updates itself in a VM.</span>
      </label>
      <div className="row-actions">
        <Button disabled={!!d?.rebuilding || rebuild.isPending} onClick={() => rebuild.mutate()}>
          {d?.rebuilding ? "Rebuilding…" : "Rebuild image"}
        </Button>
        {rebuild.error ? <span className="msg err">{rebuild.error.message}</span> : null}
        {d && !d.rebuilding && d.last_result ? (
          <span className={`msg ${d.last_result === "ok" ? "ok" : "err"}`}>
            {d.last_result === "ok" ? "Last rebuild succeeded." : `Last rebuild: ${d.last_result}.`}
          </span>
        ) : null}
      </div>
      {d && (d.rebuilding || d.last_result) && d.log_tail ? <pre className="log">{d.log_tail}</pre> : null}
    </>
  );
}

export const Fact = ({ k, v }: { k: string; v: string }) => (
  <div>
    <dt>{k}</dt>
    <dd>{v}</dd>
  </div>
);
