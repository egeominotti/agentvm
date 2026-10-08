/* A time-series line chart: one y axis from 0, recessive grid, a legend for two or more series,
   and a crosshair whose tooltip lists every series at the hovered time (keyboard too). */
import { h, svg } from "../dom.js";

const W = 300, H = 110;
const clock = t => new Date(t * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });

/** A round ceiling for the y axis, so its top label reads well. */
function niceMax(v) {
  if (v <= 0) return 1;
  const p = 10 ** Math.floor(Math.log10(v));
  return [1, 2, 2.5, 5, 10].map(m => m * p).find(m => m >= v);
}

/** `series`: [{ name, color, values }] over shared `times` (a null value is a gap). `reference`:
 *  a dashed rule for a fixed limit, `{ value, label }`. `format` turns a value into text. */
export function lineChart({ title, times, series, format, max, reference, empty = "No data yet" }) {
  const known = series.flatMap(s => s.values).filter(v => v != null);
  const fig = h("figure", { class: "chart" });
  const latest = series.map(s => [s, [...s.values].reverse().find(v => v != null)]);
  fig.append(h("figcaption", {}, h("span", { class: "chart-title" }, title),
    h("span", { class: "chart-now" }, latest.filter(([, v]) => v != null).map(([s, v]) => series.length > 1 ? `${s.name} ${format(v)}` : format(v)).join(" · "))));
  if (times.length < 2 || known.length === 0) { fig.append(h("div", { class: "chart-empty" }, empty)); return fig; }

  const top = max ?? niceMax(Math.max(...known, reference?.value ?? 0));
  const t0 = times[0], t1 = times[times.length - 1], span = Math.max(t1 - t0, 1);
  const x = t => ((t - t0) / span) * W;
  const y = v => H - (Math.min(v, top) / top) * H;
  const path = values => values.map((v, i) => (v == null ? null : `${x(times[i]).toFixed(1)},${y(v).toFixed(1)}`))
    .reduce((d, p, i, all) => (p == null ? d : d + (i && all[i - 1] != null ? "L" : "M") + p), "");
  const plot = svg("svg", { viewBox: `0 0 ${W} ${H}`, preserveAspectRatio: "none", "aria-hidden": "true" },
    ...[0.25, 0.5, 0.75].map(f => svg("line", { class: "grid", x1: 0, x2: W, y1: H * f, y2: H * f })),
    ...(reference ? [svg("line", { class: "ref", x1: 0, x2: W, y1: y(reference.value), y2: y(reference.value) })] : []),
    ...series.map(s => svg("path", { class: "series", d: path(s.values), style: `stroke:${s.color}` })));
  const hair = h("div", { class: "hair" }), tip = h("div", { class: "tip", role: "status" });
  hair.hidden = tip.hidden = true;
  const box = h("div", { class: "plot", tabindex: "0", "aria-label": `${title}: ${fig.querySelector(".chart-now").textContent}` },
    plot, h("span", { class: "y-top" }, format(top)), reference && h("span", { class: "ref-label", style: `top:${(y(reference.value) / H) * 100}%` }, reference.label), hair, tip);

  let at = times.length - 1;
  const show = i => {
    at = Math.max(0, Math.min(times.length - 1, i));
    const left = (x(times[at]) / W) * 100;
    hair.style.left = tip.style.left = `${left}%`;
    tip.classList.toggle("flip", left > 60);
    tip.replaceChildren(h("div", { class: "when" }, clock(times[at])), ...series.map(s =>
      h("div", { class: "row" }, h("i", { style: `background:${s.color}` }), h("b", {}, s.values[at] == null ? "—" : format(s.values[at])), h("span", {}, s.name))));
    hair.hidden = tip.hidden = false;
  };
  const hide = () => { hair.hidden = tip.hidden = true; };
  box.addEventListener("pointermove", e => {
    const r = box.getBoundingClientRect(), t = t0 + ((e.clientX - r.left) / r.width) * span;
    let best = 0;
    times.forEach((ti, i) => { if (Math.abs(ti - t) < Math.abs(times[best] - t)) best = i; });
    show(best);
  });
  box.addEventListener("pointerleave", hide);
  box.addEventListener("blur", hide);
  box.addEventListener("keydown", e => {
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") { e.preventDefault(); show(at + (e.key === "ArrowLeft" ? -1 : 1)); }
    if (e.key === "Escape") hide();
  });
  fig.append(box, h("div", { class: "x-axis" }, h("span", {}, clock(t0)), h("span", {}, clock(t1))));
  if (series.length > 1) fig.append(h("div", { class: "legend" }, series.map(s => h("span", {}, h("i", { style: `background:${s.color}` }), s.name))));
  return fig;
}
