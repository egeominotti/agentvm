// The Debian image every VM starts from: its state, the Claude Code it will have, its rebuild.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { GoldenStatus } from "../../api/generated/GoldenStatus";
import type { Settings } from "../../api/generated/Settings";
import { useReleases } from "../../api/queries";
import { Button } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { gb } from "../../lib/format";
import { Badge, Page, Panel, Row, Value } from "./kit";
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
    <Page
      title="VM image"
      description="Every VM starts as an instant copy of this Debian 13 image with Claude Code installed. Rebuild it to update Claude Code and the system packages; running VMs keep theirs."
    >
      <Panel title="Current image">
        <Row label="State">
          {d ? (
            <Badge tone={d.rebuilding ? "warn" : d.exists ? "ok" : "err"}>
              {d.rebuilding ? "Rebuilding" : d.exists ? "Ready" : "Missing"}
            </Badge>
          ) : null}
        </Row>
        <Row label="Claude Code">
          <Value mono>{d?.claude_version ?? "unknown"}</Value>
        </Row>
        <Row label="Built">
          <Value>{d?.built_at ? new Date(d.built_at * 1000).toLocaleString() : "never"}</Value>
        </Row>
        <Row label="Size on disk">
          <Value>{d ? gb(d.size_mb) : ""}</Value>
        </Row>
      </Panel>
      <Panel
        title="Rebuild"
        description="About 2–5 minutes. New VMs start from the new image as soon as it is ready."
        footer={
          <>
            {rebuild.error ? <span className="msg err">{rebuild.error.message}</span> : null}
            {d && !d.rebuilding && d.last_result ? (
              <span className={`msg ${d.last_result === "ok" ? "ok" : "err"}`}>
                {d.last_result === "ok" ? "Last rebuild succeeded." : `Last rebuild: ${d.last_result}.`}
              </span>
            ) : null}
            <Button variant="primary" disabled={!!d?.rebuilding || rebuild.isPending} onClick={() => rebuild.mutate()}>
              <Icon name="refresh" />
              {d?.rebuilding ? "Rebuilding…" : "Rebuild image"}
            </Button>
          </>
        }
      >
        <Row
          label="Claude Code version"
          htmlFor="set-claude-version"
          description="Installed by the next rebuild. A launch can still pick another; Claude Code never updates itself in a VM."
        >
          <select
            id="set-claude-version"
            className="text-input"
            value={s.claude_version}
            onChange={(e) => set("claude_version", e.target.value)}
          >
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
        </Row>
        {d && (d.rebuilding || d.last_result) && d.log_tail ? (
          <Row label="Log" wide>
            <pre className="log">{d.log_tail}</pre>
          </Row>
        ) : null}
      </Panel>
    </Page>
  );
}
