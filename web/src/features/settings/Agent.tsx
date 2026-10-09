// The default model, the time limit of automatic tasks and the repository suggested in New VM.
import { useState } from "react";
import type { Settings } from "../../api/generated/Settings";
import { isKnownModel, MODELS } from "../../lib/models";
import type { SetSetting } from "./useDraft";

export function Agent({ s, set }: { s: Settings; set: SetSetting }) {
  const [custom, setCustom] = useState(isKnownModel(s.model) ? "" : s.model);
  return (
    <>
      <div className="set">
        <span className="set-label">Default model</span>
        <div className="choices" role="radiogroup" aria-label="Default model">
          {MODELS.map(([id, name, desc]) => (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={s.model === id}
              onClick={() => {
                setCustom("");
                set("model", id);
              }}
            >
              <b>{name}</b>
              <span>{desc}</span>
            </button>
          ))}
        </div>
        <input
          className="text-input"
          aria-label="Another model ID"
          placeholder="Or a model ID, e.g. claude-opus-5-5"
          spellCheck={false}
          value={custom}
          onChange={(e) => {
            setCustom(e.target.value);
            const v = e.target.value.trim();
            if (v) set("model", v);
          }}
        />
        <p className="hint">
          Each launch can pick another one. Aliases always point to the newest model of the family.
        </p>
      </div>
      <div className="pair">
        <label className="set">
          <span className="set-label">Time limit for automatic tasks</span>
          <span className="unit">
            <input
              className="text-input"
              type="number"
              min={1}
              max={1440}
              value={Math.round(s.timeout_s / 60)}
              onChange={(e) => set("timeout_s", Number(e.target.value) * 60)}
            />
            minutes
          </span>
          <span className="hint">Terminals never time out: you close them.</span>
        </label>
        <label className="set">
          <span className="set-label">Default repository</span>
          <input
            className="text-input mono"
            placeholder="~/code/my-app"
            spellCheck={false}
            value={s.default_repo ?? ""}
            onChange={(e) => set("default_repo", e.target.value.trim() || null)}
          />
          <span className="hint">Suggested first in New VM.</span>
        </label>
      </div>
    </>
  );
}
