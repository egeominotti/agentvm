// Telling you that Claude waits: "(2) agentvm" in the tab's title, and a desktop notification
// when an agent goes from working to waiting while the page is in the background.
import { useEffect, useRef } from "react";
import type { TaskDto } from "../api/generated/TaskDto";
import { isWaiting, titleOf } from "../lib/task";

const PREF_KEY = "agentvm.notify";

export const notificationsSupported = () => "Notification" in window;

export function notificationsOn(): boolean {
  if (!notificationsSupported() || Notification.permission !== "granted") return false;
  try {
    return localStorage.getItem(PREF_KEY) !== "off";
  } catch {
    return true;
  }
}

/** Turns them off, or on (asking the browser's permission the first time). */
export async function setNotifications(on: boolean): Promise<void> {
  try {
    if (on) localStorage.removeItem(PREF_KEY);
    else localStorage.setItem(PREF_KEY, "off");
  } catch {
    // Not remembered: the browser's permission still decides.
  }
  if (on && Notification.permission === "default") await Notification.requestPermission();
}

export function useAttention(tasks: TaskDto[]) {
  const last = useRef(new Map<string, string | null>());
  const waiting = tasks.filter(isWaiting).length;

  useEffect(() => {
    document.title = waiting ? `(${waiting}) agentvm` : "agentvm";
  }, [waiting]);

  useEffect(() => {
    for (const t of tasks) {
      const before = last.current.get(t.id);
      last.current.set(t.id, t.activity);
      if (before === "working" && t.activity === "waiting" && document.hidden && notificationsOn()) {
        const n = new Notification("Claude is waiting for you", {
          body: titleOf(t),
          icon: `${import.meta.env.BASE_URL}logo.svg`,
          tag: t.id,
        });
        n.addEventListener("click", () => {
          window.focus();
          location.hash = `#/vm/${t.id}`;
          n.close();
        });
      }
    }
  }, [tasks]);
}
