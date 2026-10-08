/* Shared state: the machines, the server's status and the settings, loaded from the API.
 * Whoever shows them subscribes to "tasks" or "status" and repaints when they change. */
import { api } from "./api.js";
import { notifyWaiting } from "./notifications.js";
import { isEnded } from "./task.js";

export const state = { tasks: [], status: null, settings: null };
export const byId = id => state.tasks.find(t => t.id === id);

const listeners = { tasks: [], status: [] };
/** Calls `fn` after every load of `topic` ("tasks" or "status"), in subscription order. */
export function subscribe(topic, fn) { listeners[topic].push(fn); }
function publish(topic) { for (const fn of listeners[topic]) fn(); }

/** Running machines first, then finished ones; oldest first within each. */
const byLiveThenAge = (a, b) => (isEnded(a) - isEnded(b)) || a.created_at - b.created_at;

export async function loadTasks() {
  const r = await api("/api/tasks");
  if (!r.ok) return;
  notifyWaiting(r.data);
  state.tasks = r.data.sort(byLiveThenAge);
  publish("tasks");
}

/** Subscribers run even when the request fails: they repaint from the last known status. */
export async function loadStatus() {
  const r = await api("/api/status");
  if (r.ok) state.status = r.data;
  publish("status");
}

export async function loadSettings() {
  const r = await api("/api/settings");
  if (r.ok) state.settings = r.data;
}
