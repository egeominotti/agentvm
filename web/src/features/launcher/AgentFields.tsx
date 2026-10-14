import type { AgentKind } from "../../api/generated/AgentKind";
import type { AgentSelection } from "../../api/generated/AgentSelection";
import { useAgentCatalog } from "../../api/queries";
import { CLAUDE_AGENT, selection } from "../../lib/agents";

import { AuthenticationFields } from "./AuthenticationFields";

type Props = {
  agent: AgentSelection;
  custom?: AgentSelection["custom"];
  onChange: (agent: AgentSelection) => void;
  version?: boolean;
};
export function AgentFields({ agent, custom, onChange, version = true }: Props) {
  const query = useAgentCatalog();
  const catalog = query.data;
  const a = catalog === null ? CLAUDE_AGENT : agent;
  return (
    <>
      {catalog ? (
        <label>
          <span>Coding CLI</span>
          <select
            className="text-input"
            value={a.kind}
            onChange={(e) => {
              const kind = e.target.value as AgentKind;
              const next = selection(kind, catalog.agents.find((d) => d.kind === kind)?.accounts);
              if (kind === "custom") next.custom = custom ?? agent.custom;
              onChange(next);
            }}
          >
            {catalog.agents.map((d) => (
              <option key={d.kind} value={d.kind}>
                {d.name}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      {catalog ? <AuthenticationFields agent={a} catalog={catalog} onChange={onChange} /> : null}
      {version && catalog && a.kind !== "claude" && a.kind !== "custom" ? (
        <label>
          <span>CLI release</span>
          <input
            className="text-input mono"
            value={a.version ?? ""}
            placeholder="Pinned recipe (default)"
            onChange={(e) => onChange({ ...a, version: e.target.value || null })}
          />
          <small>Leave empty for the pinned installer recipe; enter latest or a release such as 1.2.3.</small>
        </label>
      ) : null}
      {query.error ? <p className="form-error">Could not read the CLI catalog: {query.error.message}</p> : null}
    </>
  );
}
