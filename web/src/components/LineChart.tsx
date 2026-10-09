// A time-series line chart: one y axis from 0, recessive grid, a legend for two or more series,
// and a crosshair whose tooltip lists every series at the hovered time (arrow keys too).
import { type KeyboardEvent, type PointerEvent, useRef, useState } from "react";
import { linePath, niceMax } from "../lib/chart";
import { clock } from "../lib/format";

// Categorical slots 1 and 2 of the reference palette, validated on the panel's dark surface.
export const BLUE = "#3987e5";
export const ORANGE = "#d95926";

export type Line = { name: string; color: string; values: (number | null)[] };

type Props = {
  title: string;
  times: number[];
  series: Line[];
  format: (v: number) => string;
  /** The top of the axis; a round number above the data when left out. */
  max?: number;
  /** A dashed rule for a fixed limit. */
  reference?: { value: number; label: string };
  empty?: string;
};

const W = 300;
const H = 96;

export function LineChart({ title, times, series, format, max, reference, empty = "No data yet" }: Props) {
  const [at, setAt] = useState<number | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const known = series.flatMap((s) => s.values).filter((v): v is number => v != null);
  const latest = series
    .map((s) => [s, s.values.findLast((v) => v != null)] as const)
    .filter((e): e is readonly [Line, number] => e[1] != null)
    .map(([s, v]) => (series.length > 1 ? `${s.name} ${format(v)}` : format(v)))
    .join(" · ");
  const head = (
    <figcaption>
      <span className="chart-title">{title}</span>
      <span className="chart-now">{latest}</span>
    </figcaption>
  );
  if (times.length === 0 || known.length === 0) {
    return (
      <figure className="chart">
        {head}
        <div className="chart-empty">{empty}</div>
      </figure>
    );
  }

  const top = max ?? niceMax(Math.max(...known, reference?.value ?? 0));
  const t0 = times[0] ?? 0;
  const t1 = times[times.length - 1] ?? t0;
  const span = t1 - t0;
  // A single moment sits in the middle of the plot.
  const x = (i: number) => (span > 0 ? (((times[i] ?? t0) - t0) / span) * W : W / 2);
  const y = (v: number) => H - (Math.min(v, top) / top) * H;
  const last = times.length - 1;

  const pick = (e: PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const t = t0 + ((e.clientX - r.left) / r.width) * span;
    let best = 0;
    times.forEach((ti, i) => {
      if (Math.abs(ti - t) < Math.abs((times[best] ?? 0) - t)) best = i;
    });
    setAt(best);
  };
  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      const step = e.key === "ArrowLeft" ? -1 : 1;
      setAt((i) => Math.max(0, Math.min(last, (i ?? last) + step)));
    } else if (e.key === "Escape") setAt(null);
  };
  const left = at == null ? 0 : (x(at) / W) * 100;

  return (
    <figure className="chart">
      {head}
      <div
        ref={box}
        className="plot"
        role="img"
        // biome-ignore lint/a11y/noNoninteractiveTabindex: the arrow keys walk through the points
        tabIndex={0}
        aria-label={`${title}: ${latest}`}
        onPointerMove={pick}
        onPointerLeave={() => setAt(null)}
        onBlur={() => setAt(null)}
        onKeyDown={keys}
      >
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden="true">
          {[0.25, 0.5, 0.75].map((f) => (
            <line key={f} className="grid" x1={0} x2={W} y1={H * f} y2={H * f} />
          ))}
          {reference ? <line className="ref" x1={0} x2={W} y1={y(reference.value)} y2={y(reference.value)} /> : null}
          {series.map((s) => (
            <path key={s.name} className="series" d={linePath(s.values, x, y)} style={{ stroke: s.color }} />
          ))}
        </svg>
        <span className="y-top">{format(top)}</span>
        {reference ? (
          <span className="ref-label" style={{ top: `${(y(reference.value) / H) * 100}%` }}>
            {reference.label}
          </span>
        ) : null}
        {at != null ? (
          <>
            <div className="hair" style={{ left: `${left}%` }} />
            <div className={`tip${left > 55 ? " flip" : ""}`} style={{ left: `${left}%` }} aria-hidden="true">
              <div className="when">{clock(times[at] ?? t0)}</div>
              {series.map((s) => {
                const v = s.values[at];
                return (
                  <div className="row" key={s.name}>
                    <i style={{ background: s.color }} />
                    <b>{v == null ? "—" : format(v)}</b>
                    <span>{s.name}</span>
                  </div>
                );
              })}
            </div>
          </>
        ) : null}
      </div>
      {/* What the arrow keys point at, for screen readers (the plot itself is an image). */}
      <span className="sr-only" aria-live="polite">
        {at != null
          ? `${clock(times[at] ?? t0)}: ${series
              .map((s) => `${s.name} ${s.values[at] == null ? "not measured" : format(s.values[at] as number)}`)
              .join(", ")}`
          : ""}
      </span>
      <div className="x-axis">
        <span>{clock(t0)}</span>
        {span > 0 ? <span>{clock(t1)}</span> : null}
      </div>
      {series.length > 1 ? (
        <div className="legend">
          {series.map((s) => (
            <span key={s.name}>
              <i style={{ background: s.color }} />
              {s.name}
            </span>
          ))}
        </div>
      ) : null}
    </figure>
  );
}
