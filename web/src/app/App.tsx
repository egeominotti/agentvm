// The dashboard's frame: the sidebar, and the screen the address asks for.

import { useTasks } from "../api/queries";
import { ToastProvider } from "../components/Toast";
import { Launcher } from "../features/launcher/Launcher";
import { MachineView } from "../features/machine/MachineView";
import { SettingsView } from "../features/settings/Settings";
import { Snapshots } from "../features/snapshots/Snapshots";
import { Wall } from "../features/wall/Wall";
import { useAttention } from "./attention";
import { useRoute } from "./router";
import { Shortcuts } from "./Shortcuts";
import { Sidebar } from "./sidebar/Sidebar";
import "../styles/app.css";
import "../styles/components.css";
import "../styles/dialog.css";
import "../styles/inspector.css";
import "../styles/machine.css";
import "../styles/outcome.css";
import "../styles/settings.css";
import "../styles/snapshots.css";
import "../styles/wall.css";
// Last: narrow windows override the rules above.
import "../styles/responsive.css";

export function App() {
  const route = useRoute();
  useAttention(useTasks().data ?? []);
  return (
    <ToastProvider>
      <div className="app">
        <Sidebar route={route} />
        <main className="view">
          {route.name === "machine" ? <MachineView key={route.id} id={route.id} /> : null}
          {route.name === "wall" ? <Wall /> : null}
          {route.name === "snapshots" ? <Snapshots /> : null}
          {route.name === "settings" ? <SettingsView section={route.section} /> : null}
        </main>
      </div>
      <Launcher />
      <Shortcuts />
    </ToastProvider>
  );
}
