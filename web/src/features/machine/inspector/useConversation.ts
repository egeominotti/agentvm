// Claude's conversation in a machine, read a page at a time from where the last read stopped,
// and again every 5 s while the machine runs. One read at a time: a poll that comes while one
// is running waits for it instead of appending the same entries twice. A failed read is said,
// keeps what was read, and can be tried again.
import { useEffect, useRef, useState } from "react";
import { api } from "../../../api/client";
import type { Conversation } from "../../../api/generated/Conversation";
import type { Cursor } from "../../../api/generated/Cursor";
import type { HistoryEntry } from "../../../api/generated/HistoryEntry";

export function useConversation(id: string, live: boolean) {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Bumped to read again after a failure (a closed machine is not polled). */
  const [attempt, setAttempt] = useState(0);
  const cursor = useRef<{ id: string; at: Cursor | null }>({ id, at: null });

  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` re-runs the read after a failure
  useEffect(() => {
    // Another machine starts from the beginning; the same one continues where it stopped.
    if (cursor.current.id !== id) {
      cursor.current = { id, at: null };
      setEntries([]);
      setLoaded(false);
      setError(null);
    }
    let gone = false;
    let reading: Promise<void> | null = null;
    const read = async () => {
      for (let more = true; more && !gone; ) {
        const at = cursor.current.at;
        const q = at ? `?cursor=${encodeURIComponent(JSON.stringify(at))}` : "";
        let page: Conversation;
        try {
          page = await api<Conversation>(`/api/tasks/${id}/claude${q}`);
        } catch (e) {
          if (!gone) setError((e as Error).message);
          return;
        }
        if (gone) return;
        setError(null);
        if (page.entries.length) setEntries((all) => [...all, ...page.entries]);
        cursor.current = { id, at: page.cursor };
        more = page.more;
      }
      if (!gone) setLoaded(true);
    };
    const load = () => {
      if (reading) return;
      reading = read().finally(() => {
        reading = null;
      });
    };
    load();
    const timer = live ? setInterval(load, 5000) : undefined;
    return () => {
      gone = true;
      clearInterval(timer);
    };
  }, [id, live, attempt]);

  return { entries, loaded, error, retry: () => setAttempt((n) => n + 1) };
}
