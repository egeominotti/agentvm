// The server says when the task list changes; the list is then re-read at once. Polling alone
// is not enough: a browser slows a background tab's timers to once a minute, so "Claude is
// waiting" would arrive late. Network events are not slowed. The polling stays as the fallback.
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { keys } from "./queries";

export function useChanges() {
  const qc = useQueryClient();
  useEffect(() => {
    if (typeof EventSource === "undefined") return;
    // Reconnects by itself after a server restart; its first event re-reads the list.
    const source = new EventSource("/api/changes");
    const reread = () => {
      qc.invalidateQueries({ queryKey: keys.tasks });
      qc.invalidateQueries({ queryKey: keys.status });
    };
    source.addEventListener("changed", reread);
    return () => source.close();
  }, [qc]);
}
