// One choice among a few, all in view (arrow keys move between them).
import type { KeyboardEvent } from "react";

type Props<T> = { label: string; value: T; options: [T, string][]; onChange: (v: T) => void; wide?: boolean };

export function Segmented<T extends string | number>({ label, value, options, onChange, wide }: Props<T>) {
  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const i = options.findIndex(([v]) => v === value);
    const next = options[(i + (e.key === "ArrowRight" ? 1 : options.length - 1)) % options.length];
    if (next) onChange(next[0]);
  };
  return (
    <div className={`seg${wide ? " wide" : ""}`} role="radiogroup" aria-label={label} onKeyDown={keys}>
      {options.map(([v, text]) => (
        <button
          key={String(v)}
          type="button"
          role="radio"
          aria-checked={v === value}
          tabIndex={v === value ? 0 : -1}
          onClick={() => onChange(v)}
        >
          {text}
        </button>
      ))}
    </div>
  );
}
