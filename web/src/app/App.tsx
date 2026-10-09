// The dashboard's frame: the sidebar, and the screen the address asks for.
import { ToastProvider } from "../components/Toast";
import { Placeholder } from "./Placeholder";
import { useRoute } from "./router";
import { Sidebar } from "./sidebar/Sidebar";
import "../styles/app.css";
import "../styles/components.css";

export function App() {
  const route = useRoute();
  return (
    <ToastProvider>
      <div className="app">
        <Sidebar route={route} />
        <main className="view">
          {route.name === "machine" ? <Placeholder what="Machine" /> : null}
          {route.name === "wall" ? <Placeholder what="Machines" /> : null}
          {route.name === "snapshots" ? <Placeholder what="Snapshots" /> : null}
          {route.name === "settings" ? <Placeholder what="Settings" /> : null}
        </main>
      </div>
    </ToastProvider>
  );
}
