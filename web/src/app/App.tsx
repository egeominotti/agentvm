// The dashboard's frame: the sidebar, and the screen the address asks for.

import { useTasks } from "../api/queries";
import { ToastProvider } from "../components/Toast";
import { Launcher } from "../features/launcher/Launcher";
import { MachineView } from "../features/machine/MachineView";
import { Wall } from "../features/wall/Wall";
import { useAttention } from "./attention";
import { Placeholder } from "./Placeholder";
import { useRoute } from "./router";
import { Sidebar } from "./sidebar/Sidebar";
import "../styles/app.css";
import "../styles/components.css";
import "../styles/dialog.css";
import "../styles/inspector.css";
import "../styles/machine.css";
import "../styles/outcome.css";
import "../styles/wall.css";

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
          {route.name === "snapshots" ? <Placeholder what="Snapshots" /> : null}
          {route.name === "settings" ? <Placeholder what="Settings" /> : null}
        </main>
      </div>
      <Launcher />
    </ToastProvider>
  );
}
