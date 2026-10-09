// A machine's terminals: Claude, the shell, and extra shells `shell-2` … `shell-9` opened beside
// them (the server's domain/terminal_session.rs has the same rule).

const EXTRA = ["shell-2", "shell-3", "shell-4", "shell-5", "shell-6", "shell-7", "shell-8", "shell-9"];

export const isExtraShell = (name: string) => EXTRA.includes(name);

export function sessionLabel(name: string): string {
  if (name === "claude") return "Claude";
  if (name === "shell") return "Shell";
  return `Shell ${name.slice("shell-".length)}`;
}

/** The lowest extra shell not open yet; `null` when all eight are. */
export function nextShell(open: string[]): string | null {
  return EXTRA.find((s) => !open.includes(s)) ?? null;
}

/** The extra shells remembered for a machine, from what was stored (anything else is dropped). */
export function parseShells(stored: string | null): string[] {
  try {
    const list: unknown = JSON.parse(stored ?? "[]");
    return Array.isArray(list) ? EXTRA.filter((s) => list.includes(s)) : [];
  } catch {
    return [];
  }
}
