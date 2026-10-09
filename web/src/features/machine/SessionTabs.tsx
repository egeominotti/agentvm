// The terminals of a running machine as tabs: Claude, the shell, the extra shells (each with its
// ×), and + for one more. Arrow keys move between them.
import type { KeyboardEvent } from "react";
import { Icon } from "../../components/Icon";
import { isExtraShell, sessionLabel } from "../../lib/shells";
import type { Sessions } from "./useSessions";

export function SessionTabs({ s }: { s: Sessions }) {
  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const i = s.tabs.indexOf(s.active);
    const next = s.tabs[(i + (e.key === "ArrowRight" ? 1 : s.tabs.length - 1)) % s.tabs.length];
    if (next) s.pick(next);
  };
  return (
    // oxlint-disable-next-line jsx-a11y/interactive-supports-focus -- the radios inside take the focus (one tab stop, arrows move)
    <div className="seg sessions" role="radiogroup" aria-label="Terminal" onKeyDown={keys}>
      {s.tabs.map((name) => (
        <span key={name} className={`session${isExtraShell(name) ? " extra" : ""}`}>
          <button
            type="button"
            role="radio"
            aria-checked={name === s.active}
            tabIndex={name === s.active ? 0 : -1}
            onClick={() => s.pick(name)}
          >
            <Icon name={name === "claude" ? "spark" : "terminal"} />
            {sessionLabel(name)}
          </button>
          {isExtraShell(name) ? (
            <button
              type="button"
              className="session-close"
              aria-label={`Close ${sessionLabel(name)}`}
              title={`Close ${sessionLabel(name)}: its shell ends in the VM`}
              onClick={() => s.close(name)}
            >
              <Icon name="close" />
            </button>
          ) : null}
        </span>
      ))}
      {s.canOpen ? (
        <button type="button" className="session-add" aria-label="New shell" title="New shell" onClick={s.open}>
          <Icon name="plus" />
        </button>
      ) : null}
    </div>
  );
}
