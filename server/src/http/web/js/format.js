/* Formatting of paths, counts, sizes, rates, money and durations. */

export const repoName = p => p.split("/").filter(Boolean).pop() || p;
export const shortPath = p => p.replace(/^\/Users\/[^/]+/, "~");
export const plural = (n, one, many = one + "s") => `${n} ${n === 1 ? one : many}`;

export function gb(mb) { return mb >= 1024 ? `${(mb / 1024).toFixed(mb >= 10240 ? 0 : 1)} GB` : `${Math.round(mb)} MB`; }

export function rate(bps) {
  if (bps < 1024) return `${bps} B/s`;
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${(bps / 1024 / 1024).toFixed(1)} MB/s`;
}

export const money = n => (!n ? "$0" : n < 0.01 ? "<$0.01" : n < 1 ? `$${n.toFixed(3)}` : n < 100 ? `$${n.toFixed(2)}` : `$${n.toFixed(0)}`);
export const tokens = n => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1000 ? `${(n / 1000).toFixed(n >= 1e4 ? 0 : 1)}k` : String(n));

export function duration(s) {
  s = Math.max(0, Math.round(s));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}
