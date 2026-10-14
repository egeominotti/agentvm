// What new VMs start with: the coding CLI, the repository suggested first, the time limit of tasks
// that run without a terminal.
import type { Settings } from "../../api/generated/Settings";
import { CLAUDE_AGENT } from "../../lib/agents";
import { AgentFields } from "../launcher/AgentFields";
import { CustomCommands } from "./CustomCommands";
import { Page, Panel, Row } from "./kit";
import type { SetSetting } from "./useDraft";

export function General({ s, set }: { s: Settings; set: SetSetting }) {
  return (
    <Page title="General" description="What every new VM starts with. Each launch can still choose otherwise.">
      <Panel title="Agent">
        <Row label="Default coding agent" wide>
          <div className="options">
            <AgentFields
              agent={s.agent ?? CLAUDE_AGENT}
              custom={s.custom_cli}
              onChange={(agent) => set("agent", agent)}
            />
          </div>
        </Row>
        <Row label="Default repository" htmlFor="set-repo" description="Suggested first in New VM: a folder or a link.">
          <input
            id="set-repo"
            className="text-input mono"
            placeholder="~/code/my-app"
            spellCheck={false}
            value={s.default_repo ?? ""}
            onChange={(e) => set("default_repo", e.target.value.trim() ? e.target.value : null)}
          />
        </Row>
        <Row
          label="Time limit for automatic tasks"
          htmlFor="set-timeout"
          description="Tasks run with a prompt and no terminal stop here. Terminals never time out."
        >
          <span className="set-number">
            <input
              id="set-timeout"
              className="text-input"
              type="number"
              min={1}
              max={1440}
              value={Math.round(s.timeout_s / 60)}
              onChange={(e) => set("timeout_s", Number(e.target.value) * 60)}
            />
            minutes
          </span>
        </Row>
      </Panel>
      <CustomCommands
        value={s.custom_cli}
        onChange={(custom) => {
          set("custom_cli", custom);
          if (s.agent?.kind === "custom") set("agent", { ...s.agent, custom });
        }}
      />
    </Page>
  );
}
