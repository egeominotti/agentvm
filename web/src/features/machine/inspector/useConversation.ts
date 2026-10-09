// Claude's conversation in a machine, read a page at a time from where the last read stopped,
// and again every 5 s while the machine runs. One read at a time: a poll that comes while one
// is running waits for it instead of appending the same entries twice.
import { useEffect, useRef, useState } from "react";
import { api } from "../../../api/client";
import type { Conversation } from "../../../api/generated/Conversation";
import type { Cursor } from "../../../api/generated/Cursor";
import type { HistoryEntry } from "../../../api/generated/HistoryEntry";

export function useConversation(id: string, live: boolean) {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loaded, setLoaded] = useState(false);
  const cursor = useRef<Cursor | null>(null);

  useEffect(() => {
    let gone = false;
    let reading: Promise<void> | null = null;
    cursor.current = null;
    setEntries([]);
    setLoaded(false);
    const read = async () => {
      for (let more = true; more && !gone; ) {
        const q = cursor.current ? `?cursor=${encodeURIComponent(JSON.stringify(cursor.current))}` : "";
        const page = await api<Conversation>(`/api/tasks/${id}/claude${q}`).catch(() => null);
        if (!page || gone) break;
        if (page.entries.length) setEntries((all) => [...all, ...page.entries]);
        cursor.current = page.cursor;
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
  }, [id, live]);

  return { entries, loaded };
}
