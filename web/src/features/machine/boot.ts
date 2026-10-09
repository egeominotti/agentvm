// The boot of a machine as steps, from its boot log: real events with real timings.
import type { TaskDto } from "../../api/generated/TaskDto";

export type BootStep = { label: string; done: boolean; failed?: boolean; detail: string };

export function bootSteps(t: TaskDto): { steps: BootStep[]; current: number } {
  const host: Record<string, string> = {};
  const guest: { at: number; text: string }[] = [];
  for (const line of t.boot_log) {
    const h = line.match(/^host: (repository (?:packed|shared)|disk ready) in (\d+) ms/);
    if (h?.[1] && h[2]) {
      host[h[1].startsWith("repository") ? "repository" : h[1]] = `${h[2]} ms`;
      continue;
    }
    const g = line.match(/^\[([\d.]+)s\] (.*)$/);
    if (g?.[1] && g[2] !== undefined) guest.push({ at: Number(g[1]), text: g[2] });
  }
  const seen = (re: RegExp) => guest.find((g) => re.test(g.text));
  const secs = (g?: { at: number }) => (g ? `${g.at.toFixed(1)} s` : "");
  const queued = t.status.state === "queued";
  const steps: BootStep[] = [
    { label: "VM slot reserved", done: !queued, detail: queued ? "waiting for a free slot" : "" },
    { label: "Disk cloned from the image", done: !!host["disk ready"], detail: host["disk ready"] ?? "" },
    { label: "Debian booted", done: !!seen(/^job start/), detail: secs(seen(/^job start/)) },
    { label: "Repository checked out", done: !!seen(/^repo ready/), detail: secs(seen(/^repo ready/)) },
    { label: "Network up", done: !!seen(/^network ready/), detail: secs(seen(/^network ready/)) },
  ];
  if (seen(/^running \.agentvm\/setup\.sh/)) {
    const end = seen(/^setup (done|failed)/);
    const failed = !!end && /failed/.test(end.text);
    const detail = end ? (failed ? "failed, see setup.log" : secs(end)) : "running";
    steps.push({ label: "Repository setup (.agentvm/setup.sh)", done: !!end, failed, detail });
  }
  const install = seen(/^installing Claude Code/);
  if (install) {
    const installed = seen(/^Claude Code \d/);
    steps.push({
      label: install.text.replace(/^installing/, "Installing"),
      done: !!installed,
      detail: secs(installed),
    });
  }
  const ready = seen(t.interactive ? /^terminal ready/ : /^network ready/);
  steps.push({
    label: t.interactive ? "Claude Code ready" : "Agent started",
    done: t.ready || !!ready,
    detail: secs(ready),
  });
  const current = steps.findIndex((s) => !s.done);
  return { steps, current: current < 0 ? steps.length : current };
}
