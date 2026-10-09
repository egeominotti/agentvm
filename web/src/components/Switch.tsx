// An on/off switch with its words beside it.
import type { ReactNode } from "react";

type Props = { on: boolean; onChange: (on: boolean) => void; label: ReactNode; hint?: ReactNode; disabled?: boolean };

export function Switch({ on, onChange, label, hint, disabled }: Props) {
  return (
    <label className="switch-row">
      <button
        type="button"
        role="switch"
        aria-checked={on}
        className="switch"
        disabled={disabled}
        onClick={() => onChange(!on)}
      >
        <i />
      </button>
      <span>
        <b>{label}</b>
        {hint ? <span className="hint">{hint}</span> : null}
      </span>
    </label>
  );
}
