// The repository to launch on: recent ones suggested as you type (arrows and Enter pick one),
// and the server's check of it right under the field, before anything is launched.
import { type KeyboardEvent, type Ref, useId, useState } from "react";
import type { RepoCheck } from "../../api/generated/RepoCheck";
import { useRepoCheck } from "../../api/queries";
import { repoName } from "../../lib/format";
import { useDebounced } from "../../lib/useDebounced";

type Props = {
  value: string;
  onChange: (v: string) => void;
  onPicked: () => void;
  recent: string[];
  inputRef?: Ref<HTMLInputElement>;
  check: RepoCheck | undefined;
};

export function RepoField({ value, onChange, onPicked, recent, inputRef, check }: Props) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const listId = useId();
  const q = value.trim().toLowerCase();
  const matches = recent.filter((p) => !q || (p.toLowerCase().includes(q) && p.toLowerCase() !== q));
  const showList = open && matches.length > 0;
  const pick = (p: string) => {
    onChange(p);
    setOpen(false);
    onPicked();
  };
  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (!showList) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActive((i) => (i + step + matches.length) % matches.length);
    } else if (e.key === "Enter" && !e.metaKey && !e.ctrlKey) {
      const p = matches[active];
      if (p) {
        e.preventDefault();
        pick(p);
      }
    } else if (e.key === "Escape") {
      e.stopPropagation();
      setOpen(false);
    }
  };
  return (
    <div className="repo-field">
      <label className="field-label" htmlFor={`${listId}-input`}>
        Repository
      </label>
      <input
        id={`${listId}-input`}
        ref={inputRef}
        className="repo-input"
        value={value}
        placeholder="~/code/my-app"
        autoComplete="off"
        spellCheck={false}
        role="combobox"
        aria-expanded={showList}
        aria-controls={listId}
        aria-activedescendant={showList ? `${listId}-${active}` : undefined}
        onChange={(e) => {
          onChange(e.target.value);
          setOpen(true);
          setActive(0);
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onKeyDown={keys}
      />
      {showList ? (
        <div className="suggest" id={listId} role="listbox">
          {matches.map((p, i) => (
            <div
              key={p}
              id={`${listId}-${i}`}
              role="option"
              tabIndex={-1}
              aria-selected={i === active}
              className={i === active ? "active" : ""}
              onMouseDown={(e) => {
                e.preventDefault();
                pick(p);
              }}
            >
              <span>{repoName(p)}</span>
              <small>{p}</small>
            </div>
          ))}
        </div>
      ) : null}
      <RepoStatus path={value} check={check} />
    </div>
  );
}

/** The server's word on the repository; `check` is for the path as it was when typing stopped. */
function RepoStatus({ path, check: c }: { path: string; check: RepoCheck | undefined }) {
  if (!path.trim())
    return <p className="repo-status">The VM gets a fresh clone of it: your files are never touched.</p>;
  if (!c) return <p className="repo-status">Checking…</p>;
  if (!c.ok) return <p className="repo-status bad">{c.error}</p>;
  return (
    <p className="repo-status good">
      <span className="ok-mark" aria-hidden="true" />
      {c.name}
      {c.branch ? ` · ${c.branch}` : " · detached HEAD"}
      {c.sha ? ` · ${c.sha.slice(0, 7)}` : ""}
    </p>
  );
}

/** The check of `path`, asked once typing has stopped for 300 ms; `undefined` until it answers
 *  for the path as it is now. */
export function useSettledCheck(path: string): RepoCheck | undefined {
  const settled = useDebounced(path.trim(), 300);
  const check = useRepoCheck(settled);
  if (settled !== path.trim()) return undefined;
  // The server could not check it: say so, instead of "Checking…" for ever.
  if (check.error) {
    return { ok: false, path: settled, name: "", branch: null, sha: null, error: check.error.message };
  }
  return check.data;
}
