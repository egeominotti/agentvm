/* Telling the user that Claude waits for them: desktop notifications and the tab title. */
import { isWaiting, titleOf } from "./task.js";

const PREF_KEY = "agentvm.notify";

export const notificationsSupported = () => "Notification" in window;
export const notificationsOn = () => notificationsSupported() && Notification.permission === "granted" && localStorage.getItem(PREF_KEY) !== "off";

/** Turns them off, or on (asking the browser's permission the first time). */
export async function toggleNotifications() {
  if (notificationsOn()) localStorage.setItem(PREF_KEY, "off");
  else { localStorage.removeItem(PREF_KEY); if (Notification.permission === "default") await Notification.requestPermission(); }
}

const lastActivity = new Map();

/** A notification for every agent that just went from working to waiting while the page is in the background. */
export function notifyWaiting(tasks) {
  for (const t of tasks) {
    const before = lastActivity.get(t.id);
    lastActivity.set(t.id, t.activity);
    if (before === "working" && t.activity === "waiting" && document.hidden && notificationsOn()) {
      const n = new Notification("Claude is waiting for you", { body: titleOf(t), icon: "/logo.svg", tag: t.id });
      n.onclick = () => { window.focus(); location.hash = `#/vm/${t.id}`; n.close(); };
    }
  }
}

/** "(2) agentvm" while two agents wait for the user. */
export function badgeTitle(tasks) {
  const waiting = tasks.filter(isWaiting).length;
  document.title = waiting ? `(${waiting}) agentvm` : "agentvm";
}
