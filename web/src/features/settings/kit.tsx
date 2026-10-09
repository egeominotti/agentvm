// What every settings page is made of, so they all read the same way: a page with its title and
// purpose; panels of rows, each with what the setting is on the left and its control on the
// right; badges for states; a danger panel for what cannot be undone.
import type { ReactNode } from "react";

export function Page({ title, description, children }: { title: string; description: ReactNode; children: ReactNode }) {
  return (
    <article className="set-page">
      <header className="set-page-head">
        <h1>{title}</h1>
        <p>{description}</p>
      </header>
      {children}
    </article>
  );
}

type PanelProps = {
  title?: string;
  description?: ReactNode;
  /** Actions at the bottom (Save, Test…), right-aligned. */
  footer?: ReactNode;
  danger?: boolean;
  children: ReactNode;
};

export function Panel({ title, description, footer, danger, children }: PanelProps) {
  return (
    <section className={`set-group${danger ? " danger" : ""}`}>
      {title ? (
        <header className="set-group-head">
          <h2>{title}</h2>
          {description ? <p>{description}</p> : null}
        </header>
      ) : null}
      <div className="set-panel">
        {children}
        {footer ? <footer className="set-panel-foot">{footer}</footer> : null}
      </div>
    </section>
  );
}

type RowProps = {
  label: ReactNode;
  description?: ReactNode;
  /** The control's id, so the label focuses it. */
  htmlFor?: string;
  /** The control takes the full width under the label (a bar, a list, a log). */
  wide?: boolean;
  children?: ReactNode;
};

export function Row({ label, description, htmlFor, wide, children }: RowProps) {
  const Label = htmlFor ? "label" : "div";
  return (
    <div className={`set-row${wide ? " wide" : ""}`}>
      <Label className="set-row-text" {...(htmlFor ? { htmlFor } : {})}>
        <span className="set-row-label">{label}</span>
        {description ? <span className="set-row-desc">{description}</span> : null}
      </Label>
      {children !== undefined ? <div className="set-row-control">{children}</div> : null}
    </div>
  );
}

export type Tone = "ok" | "warn" | "err" | "off";

export function Badge({ tone, children }: { tone: Tone; children: ReactNode }) {
  return <span className={`badge ${tone === "off" ? "" : tone}`.trim()}>{children}</span>;
}

/** A read-only value in a row: a size, a version, a date. */
export function Value({ children, mono }: { children: ReactNode; mono?: boolean }) {
  return <span className={`set-value${mono ? " mono" : ""}`}>{children}</span>;
}

/** An on/off switch for a row (the row's label names it). */
export function Toggle({
  on,
  onChange,
  label,
  disabled,
}: {
  on: boolean;
  onChange: (on: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      className="switch"
      disabled={disabled}
      onClick={() => onChange(!on)}
    >
      <i />
    </button>
  );
}
