// Tokens per model call, counted once per message: Claude Code repeats them on each block.
import type { HistoryEntry } from "../../../api/generated/HistoryEntry";
import type { TurnUsage } from "../../../api/generated/TurnUsage";

export function turns(entries: HistoryEntry[]): (TurnUsage & { at: number })[] {
  const seen = new Set<string>();
  const out: (TurnUsage & { at: number })[] = [];
  for (const e of entries) {
    if (!e.usage || !e.message_id || seen.has(e.message_id)) continue;
    seen.add(e.message_id);
    const at = Date.parse(e.at) / 1000;
    if (!Number.isNaN(at)) out.push({ at, ...e.usage });
  }
  return out;
}
