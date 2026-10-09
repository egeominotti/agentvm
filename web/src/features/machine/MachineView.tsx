// One machine, full size: its terminal always whole, the details beside it. A closed machine
// shows what it produced instead of its terminal.
import { type DragEvent, useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { useTask, useTasks } from "../../api/queries";
import { TOGGLE_PANEL_EVENT } from "../../app/keys";
import { useToast } from "../../components/Toast";
import { isEnded } from "../../lib/task";
import { useMachineActions } from "./actions";
import { BootMark, BootPanel } from "./BootPanel";
import { setupFailed } from "./boot";
import { Inspector, type InspectorTab } from "./inspector/Inspector";
import { DiffView } from "./outcome/DiffView";
import { Outcome } from "./outcome/Outcome";
import { PortsBar } from "./PortsBar";
import { type Session, Toolbar } from "./Toolbar";
import { typed, uploadFiles } from "./terminal/files";
import { type TerminalHandle, VmTerminal } from "./terminal/VmTerminal";

const PANEL_KEY = "agentvm.inspector";

/** In a narrow window the details cover the terminal: they start closed, and open on demand. */
function storedPanel(): boolean {
  if (window.innerWidth < 1100) return false;
  try {
    return localStorage.getItem(PANEL_KEY) !== "off";
  } catch {
    return true;
  }
}

export function MachineView({ id }: { id: string }) {
  const list = useTasks();
  const t = useTask(id);
  const actions = useMachineActions(id);
  const say = useToast();
  const [session, setSession] = useState<Session>("claude");
  const [shellOpened, setShellOpened] = useState(false);
  const [panel, setPanel] = useState(storedPanel);
  const [tab, setTab] = useState<InspectorTab>("telemetry");
  const [dropping, setDropping] = useState(false);
  const terms = { claude: useRef<TerminalHandle>(null), shell: useRef<TerminalHandle>(null) };

  const state = t?.status.state;
  const ended = t ? isEnded(t) : false;
  // A failure explains itself at once; a closed machine's conversation is what is left of its work;
  // an automatic task has no terminal, so its conversation is the way to follow it.
  useEffect(() => {
    if (state === "failed") setTab("diagnostics");
    else if (ended || t?.interactive === false) setTab("claude");
  }, [state, ended, t?.interactive]);

  // ⌘J anywhere shows or hides the details.
  useEffect(() => {
    const toggle = () =>
      setPanel((p) => {
        try {
          localStorage.setItem(PANEL_KEY, p ? "off" : "on");
        } catch {
          // Not remembered: it still works.
        }
        return !p;
      });
    window.addEventListener(TOGGLE_PANEL_EVENT, toggle);
    return () => window.removeEventListener(TOGGLE_PANEL_EVENT, toggle);
  }, []);

  if (!t) {
    return (
      <div className="empty-state">
        <b>{list.isSuccess ? "Machine not found" : list.error ? "Cannot reach agentvm" : "Loading…"}</b>
        {list.isSuccess ? <span>It may have been removed from the list.</span> : null}
        {list.error ? <span>{list.error.message} Retrying…</span> : null}
      </div>
    );
  }

  const running = state === "running";
  const showPanel = panel;
  const togglePanel = () => window.dispatchEvent(new Event(TOGGLE_PANEL_EVENT));
  // Shown at once, then focused in the same click: keys typed right after go to the terminal.
  const pick = (s: Session) => {
    flushSync(() => {
      setSession(s);
      if (s === "shell") setShellOpened(true);
    });
    terms[s].current?.focus();
  };
  const accepts = (e: DragEvent) => running && t.interactive && e.dataTransfer.types.includes("Files");
  const drop = async (e: DragEvent) => {
    e.preventDefault();
    setDropping(false);
    const paths = await uploadFiles(id, [...e.dataTransfer.files], say);
    if (paths.length) terms[session].current?.paste(typed(paths));
  };

  return (
    <div className={`machine${showPanel ? " with-panel" : ""}`}>
      <section className="stage">
        <Toolbar
          task={t}
          session={session}
          onSession={pick}
          actions={actions}
          inspector={showPanel}
          onInspector={togglePanel}
        />
        {!ended ? <PortsBar ports={t.ports} /> : null}
        {!ended && setupFailed(t) ? (
          <div className="banner warn" role="alert">
            <span>
              The repository's setup (<code>.agentvm/setup.sh</code>) failed: this VM runs without what it installs.
            </span>
            <button
              type="button"
              className="link"
              onClick={() => {
                setTab("diagnostics");
                if (!panel) togglePanel();
              }}
            >
              Show the log
            </button>
          </div>
        ) : null}
        {ended ? (
          <div className="result">
            <Outcome task={t} />
            {state === "done" ? <DiffView id={id} /> : null}
          </div>
        ) : (
          <section
            className={`screen${dropping ? " dropping" : ""}`}
            aria-label="Terminal"
            onDragOver={(e) => {
              if (!accepts(e)) return;
              e.preventDefault();
              setDropping(true);
            }}
            onDragLeave={(e) => {
              if (!e.currentTarget.contains(e.relatedTarget as Node)) setDropping(false);
            }}
            onDrop={drop}
          >
            {t.interactive ? (
              <>
                <VmTerminal ref={terms.claude} id={id} session="claude" live={running} visible={session === "claude"} />
                {shellOpened ? (
                  <VmTerminal ref={terms.shell} id={id} session="shell" live={running} visible={session === "shell"} />
                ) : null}
              </>
            ) : null}
            {!t.interactive ? (
              <div className="overlay">
                <div className="overlay-note">
                  <BootMark />
                  <b>An automatic task: no terminal</b>
                  <span>Follow Claude's work in the Claude panel.</span>
                </div>
              </div>
            ) : state === "collecting" ? (
              <div className="overlay">
                <div className="overlay-note" role="status">
                  <BootMark />
                  <b>Saving the work and shutting down…</b>
                  <span>The commits go to {t.branch} in your repository.</span>
                </div>
              </div>
            ) : !running || !t.ready ? (
              <div className="overlay">
                <BootPanel task={t} />
              </div>
            ) : null}
            <div className="drop-hint">
              <b>Drop to copy into the VM</b>
              <span>The files go to /mnt/job/uploads and their paths are typed in the terminal.</span>
            </div>
          </section>
        )}
      </section>
      {showPanel ? <Inspector key={String(ended)} task={t} tab={tab} onTab={setTab} /> : null}
    </div>
  );
}
