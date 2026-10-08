import { svg } from "../dom.js";

/** A small line chart of `values` between 0 and `max`; `fluid` stretches it to its container's width. */
export function sparkline(values, { width = 120, height = 26, max = 100, color = "var(--brand)", fluid = false } = {}) {
  const pts = values.length ? values : [0];
  const step = width / Math.max(pts.length - 1, 1);
  const y = v => height - 1 - (Math.min(v, max) / max) * (height - 2);
  const line = pts.map((v, i) => `${i ? "L" : "M"}${(i * step).toFixed(1)},${y(v).toFixed(1)}`).join("");
  const area = `${line}L${((pts.length - 1) * step).toFixed(1)},${height}L0,${height}Z`;
  return svg("svg", { class: "spark", width: fluid ? "100%" : width, height, viewBox: `0 0 ${width} ${height}`, preserveAspectRatio: "none", style: `--sc:${color}`, "aria-hidden": "true" },
    svg("path", { class: "area", d: area }), svg("path", { class: "line", d: line }));
}
