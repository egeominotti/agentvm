// One choice among a few, all in view (arrow keys move between them).
import type { KeyboardEvent, ReactNode } from "react";

type Props<T> = { label: string; value: T; options: [T, ReactNode][]; onChange: (v: T) => void; wide?: boolean };

export function Segmented<T extends string | number>({ label, value, options, onChange, wide }: Props<T>) {
  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const i = options.findIndex(([v]) => v === value);
    const next = options[(i + (e.key === "ArrowRight" ? 1 : options.length - 1)) % options.length];
    if (next) onChange(next[0]);
  };
  // With no option chosen yet, the first one still takes the keyboard.
  const focusable = options.some(([v]) => v === value) ? value : options[0]?.[0];
  return (
    // oxlint-disable-next-line jsx-a11y/interactive-supports-focus -- the radios inside take the focus (one tab stop, arrows move)
    <div className={`seg${wide ? " wide" : ""}`} role="radiogroup" aria-label={label} onKeyDown={keys}>
      {options.map(([v, text]) => (
        <button
          key={String(v)}
          type="button"
          role="radio"
          aria-checked={v === value}
          tabIndex={v === focusable ? 0 : -1}
          onClick={() => onChange(v)}
        >
          {text}
        </button>
      ))}
    </div>
  );
}
