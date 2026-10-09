// The page's keyboard: "?" lists the shortcuts, "g" then a letter goes to a screen, ⌘J shows or
// hides a machine's details. Keys typed into a field or a terminal are left alone.
import * as Dialog from "@radix-ui/react-dialog";
import { useEffect, useState } from "react";
import { isTyping, SHORTCUTS, TOGGLE_PANEL_EVENT } from "./keys";
import { go } from "./router";

const GO: Record<string, string> = { m: "#/wall", s: "#/snapshots", ",": "#/settings" };

export function Shortcuts() {
  const [help, setHelp] = useState(false);
  useEffect(() => {
    let leader = 0;
    const keydown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "j") {
        e.preventDefault();
        window.dispatchEvent(new Event(TOGGLE_PANEL_EVENT));
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey || isTyping(e.target)) return;
      if (e.key === "?") {
        e.preventDefault();
        setHelp(true);
      } else if (e.key === "g") {
        leader = Date.now();
      } else if (Date.now() - leader < 1200 && GO[e.key]) {
        e.preventDefault();
        go(GO[e.key] as string);
        leader = 0;
      }
    };
    document.addEventListener("keydown", keydown);
    return () => document.removeEventListener("keydown", keydown);
  }, []);
  return (
    <Dialog.Root open={help} onOpenChange={setHelp}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog shortcuts" aria-describedby={undefined}>
          <header className="dialog-head">
            <Dialog.Title>Keyboard shortcuts</Dialog.Title>
            <Dialog.Close className="icon-btn" aria-label="Close">
              ✕
            </Dialog.Close>
          </header>
          <dl>
            {SHORTCUTS.map(([keys, what]) => (
              <div key={keys}>
                <dt>
                  {keys.split(" ").map((k) => (
                    <kbd key={k}>{k}</kbd>
                  ))}
                </dt>
                <dd>{what}</dd>
              </div>
            ))}
          </dl>
          <p className="hint">In a terminal every key goes to the VM; these work everywhere else.</p>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
