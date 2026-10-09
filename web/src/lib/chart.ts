// The geometry of a line chart: a round top for the y axis and the path of a series.

/** A round ceiling for the y axis, so its top label reads well. */
export function niceMax(v: number): number {
  if (v <= 0) return 1;
  const p = 10 ** Math.floor(Math.log10(v));
  return [1, 2, 2.5, 5, 10].map((m) => m * p).find((m) => m >= v) ?? v;
}

/** An SVG path through the values (`null` is a gap). A lone point gets a dot-sized segment, so
 *  a round line cap draws it: one model call is still something to see. */
export function linePath(values: (number | null)[], x: (i: number) => number, y: (v: number) => number): string {
  let d = "";
  values.forEach((v, i) => {
    if (v == null) return;
    const p = `${x(i).toFixed(1)},${y(v).toFixed(1)}`;
    d += (i > 0 && values[i - 1] != null ? "L" : "M") + p;
  });
  const points = values.filter((v) => v != null).length;
  return points === 1 ? `${d}h0.1` : d;
}

/** Whether a chart has a line to draw: at least one series with values at two moments. */
export function drawable(times: number[], series: (number | null)[][]): boolean {
  return times.length >= 2 && series.some((values) => values.filter((v) => v != null).length >= 2);
}
