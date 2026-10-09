// The terminals of one machine: which one is in view, which are open, and opening or closing the
// extra shells. Each terminal is created only when first shown (a hidden one keeps its stream and
// is not drawn), and the open shells are remembered per machine, so a reload brings them back.
import { createRef, type RefObject, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { api } from "../../api/client";
import { useToast } from "../../components/Toast";
import { isExtraShell, nextShell, parseShells, sessionLabel } from "../../lib/shells";
import type { TerminalHandle } from "./terminal/VmTerminal";

const key = (id: string) => `agentvm.shells.${id}`;

function storedShells(id: string): string[] {
  try {
    return parseShells(localStorage.getItem(key(id)));
  } catch {
    return [];
  }
}

function remember(id: string, shells: string[]) {
  try {
    if (shells.length) localStorage.setItem(key(id), JSON.stringify(shells));
    else localStorage.removeItem(key(id));
  } catch {
    // Not remembered: the shells stay open in the VM, a reload just shows fewer tabs.
  }
}

export type Sessions = ReturnType<typeof useSessions>;

export function useSessions(id: string) {
  const say = useToast();
  const [active, setActive] = useState("claude");
  const [extra, setExtra] = useState(() => storedShells(id));
  // Terminals already created; the first shell joins when it is first chosen.
  const [mounted, setMounted] = useState(() => new Set(["claude", ...storedShells(id)]));
  const refs = useRef(new Map<string, RefObject<TerminalHandle | null>>());
  /** One stable ref per terminal: the memoized terminals never re-render for it. */
  const refOf = (name: string) => {
    let r = refs.current.get(name);
    if (!r) {
      r = createRef<TerminalHandle>();
      refs.current.set(name, r);
    }
    return r;
  };

  const tabs = ["claude", "shell", ...extra];
  // Shown and created in the same click, then focused: keys typed right after go to it.
  const pick = (name: string) => {
    flushSync(() => {
      setActive(name);
      setMounted((m) => (m.has(name) ? m : new Set(m).add(name)));
    });
    refOf(name).current?.focus();
  };

  const open = () => {
    const name = nextShell(extra);
    if (!name) return say("Nine shells are open: close one to open another", "err");
    const next = [...extra, name].sort();
    flushSync(() => setExtra(next));
    remember(id, next);
    pick(name);
  };

  const close = async (name: string) => {
    if (!isExtraShell(name)) return;
    try {
      await api(`/api/tasks/${id}/pty/${name}`, "DELETE");
    } catch (e) {
      // A VM started before shells could be closed: the tab still goes, its shell stays in the VM.
      say(`${sessionLabel(name)} closed here: ${(e as Error).message}`, "err");
    }
    const next = extra.filter((s) => s !== name);
    setExtra(next);
    remember(id, next);
    setMounted((m) => {
      const n = new Set(m);
      n.delete(name);
      return n;
    });
    refs.current.delete(name);
    if (active === name) pick(tabs[Math.max(0, tabs.indexOf(name) - 1)] ?? "claude");
  };

  return { active, tabs, mounted, canOpen: nextShell(extra) !== null, refOf, pick, open, close };
}
