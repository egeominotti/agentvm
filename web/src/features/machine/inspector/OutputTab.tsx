import { useEffect, useRef, useState } from "react";
import { api } from "../../../api/client";
import { Button } from "../../../components/Button";
import { appendOutput, outputText, type OutputItem, type OutputPage } from "./output";

export function OutputTab({ id, live }: { id: string; live: boolean }) {
  const [{ items, gap }, setOutput] = useState<{ items: OutputItem[]; gap: boolean }>({ items: [], gap: false });
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const cursor = useRef({ id, at: 0 });
  useEffect(() => {
    if (cursor.current.id !== id) {
      cursor.current = { id, at: 0 };
      setOutput({ items: [], gap: false });
      setLoaded(false);
    }
    let gone = false;
    let reading = false;
    const read = async () => {
      if (reading) return;
      reading = true;
      try {
        // oxlint-disable-next-line no-unmodified-loop-condition -- cleanup changes gone while a page is awaited
        for (let more = true; more && !gone;) {
          const page = await api<OutputPage>(`/api/tasks/${id}/output?cursor=${cursor.current.at}&limit=100`);
          if (gone) return;
          setOutput((previous) => ({
            items: appendOutput(previous.items, page.items),
            gap: previous.gap || page.dropped > 0 || previous.items.length + page.items.length > 500,
          }));
          const previous = cursor.current.at;
          cursor.current.at = page.next_cursor;
          more = page.more && page.next_cursor > previous;
        }
        if (!gone) {
          setLoaded(true);
          setError(null);
        }
      } catch (err) {
        if (!gone) setError((err as Error).message);
      } finally {
        reading = false;
      }
    };
    read();
    const timer = live ? setInterval(read, 2000) : undefined;
    return () => {
      gone = true;
      clearInterval(timer);
    };
  }, [id, live, attempt]);
  return (
    <div className="claude-tab">
      <div className="tab-actions">
        <Button size="sm" onClick={() => setAttempt((n) => n + 1)}>
          Refresh
        </Button>
      </div>
      {error ? <p className="msg err">Could not read output: {error}</p> : null}
      {gap ? <p className="hint">Earlier output was truncated. Showing the latest retained events.</p> : null}
      {items.map((entry) => (
        <div className={`turn ${entry.item.type === "state" ? "user" : "claude"}`} key={entry.seq}>
          <pre>
            {entry.item.type === "state"
              ? `State: ${entry.item.data.state.replaceAll("_", " ")}`
              : outputText(entry.item.data)}
          </pre>
        </div>
      ))}
      {!items.length && !error ? (
        <p className="hint">{loaded ? "No agent output yet." : "Reading agent output…"}</p>
      ) : null}
    </div>
  );
}
