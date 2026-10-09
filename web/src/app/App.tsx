// The dashboard's frame: the sidebar, and the screen the address asks for.
import { ToastProvider } from "../components/Toast";
import { MachineView } from "../features/machine/MachineView";
import { Placeholder } from "./Placeholder";
import { useRoute } from "./router";
import { Sidebar } from "./sidebar/Sidebar";
import "../styles/app.css";
import "../styles/components.css";
import "../styles/inspector.css";
import "../styles/machine.css";
import "../styles/outcome.css";

export function App() {
  const route = useRoute();
  return (
    <ToastProvider>
      <div className="app">
        <Sidebar route={route} />
        <main className="view">
          {route.name === "machine" ? <MachineView key={route.id} id={route.id} /> : null}
          {route.name === "wall" ? <Placeholder what="Machines" /> : null}
          {route.name === "snapshots" ? <Placeholder what="Snapshots" /> : null}
          {route.name === "settings" ? <Placeholder what="Settings" /> : null}
        </main>
      </div>
    </ToastProvider>
  );
}
