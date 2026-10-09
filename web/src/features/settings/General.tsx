// What new VMs start with: the model, the repository suggested first, the time limit of tasks
// that run without a terminal.
import { useState } from "react";
import type { Settings } from "../../api/generated/Settings";
import { isKnownModel, MODELS } from "../../lib/models";
import { Page, Panel, Row } from "./kit";
import type { SetSetting } from "./useDraft";

const OTHER = "__other";

export function General({ s, set }: { s: Settings; set: SetSetting }) {
  const [other, setOther] = useState(!isKnownModel(s.model));
  return (
    <Page title="General" description="What every new VM starts with. Each launch can still choose otherwise.">
      <Panel title="Agent">
        <Row
          label="Default model"
          htmlFor="set-model"
          description={`${MODELS.find(([id]) => id === s.model)?.[2] ?? "A model ID of your choice"}. Aliases point to the newest of their family.`}
        >
          <select
            id="set-model"
            className="text-input"
            value={other ? OTHER : s.model}
            onChange={(e) => {
              const v = e.target.value;
              setOther(v === OTHER);
              if (v !== OTHER) set("model", v);
            }}
          >
            {MODELS.map(([id, name]) => (
              <option key={id} value={id}>
                {name}
              </option>
            ))}
            <option value={OTHER}>Another model ID…</option>
          </select>
        </Row>
        {other ? (
          <Row label="Model ID" htmlFor="set-model-id" description="As Claude Code takes it, e.g. claude-opus-5-5.">
            <input
              id="set-model-id"
              className="text-input mono"
              placeholder="claude-opus-5-5"
              spellCheck={false}
              defaultValue={isKnownModel(s.model) ? "" : s.model}
              onChange={(e) => e.target.value.trim() && set("model", e.target.value.trim())}
            />
          </Row>
        ) : null}
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
    </Page>
  );
}
