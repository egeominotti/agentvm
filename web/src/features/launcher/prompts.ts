// What the launcher sends: one task per VM, and the repositories used lately.

/** The whole text as one task, or with `perLine` each line of it (list markers dropped). An
 *  empty task still launches one VM: you work in its terminal. */
export function splitPrompts(text: string, perLine: boolean): string[] {
  const whole = text.trim();
  if (!perLine) return [whole];
  const lines = whole
    .split("\n")
    .map((l) => l.replace(/^\s*(?:[-*•]|\d+[.)])\s+/, "").trim())
    .filter(Boolean);
  return lines.length ? lines : [""];
}

/** `path` first, then the others, without duplicates, at most 8. */
export const mergeRecent = (path: string, recent: string[]) => [path, ...recent.filter((p) => p !== path)].slice(0, 8);

const RECENT_KEY = "agentvm.recentRepos";

export function storedRecent(): string[] {
  try {
    const list: unknown = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((p): p is string => typeof p === "string") : [];
  } catch {
    return [];
  }
}

export function rememberRepo(path: string) {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(mergeRecent(path, storedRecent())));
  } catch {
    // Not remembered: the field still takes any path.
  }
}
