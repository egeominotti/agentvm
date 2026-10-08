/* agentvm dashboard: a wall of live machines, a focused machine view, settings. */
import { badgeTitle } from "./notifications.js";
import { route, startRouter, updateView } from "./router.js";
import { renderHost, renderSide } from "./sidebar.js";
import { loadSettings, loadStatus, loadTasks, state, subscribe } from "./state.js";
import { initLauncher, prefillLauncher, updateLaunch } from "./views/launcher/launcher.js";

subscribe("tasks", () => badgeTitle(state.tasks));
subscribe("tasks", renderSide);
subscribe("tasks", updateView);
subscribe("status", renderHost);
subscribe("status", updateLaunch);
initLauncher();
startRouter();

async function start() {
  await Promise.all([loadSettings(), loadStatus()]);
  await loadTasks();
  prefillLauncher();
  route();
  setInterval(loadTasks, 1000);
  setInterval(loadStatus, 3000);
}
start();
