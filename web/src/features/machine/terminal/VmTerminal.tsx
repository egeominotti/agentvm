// A terminal attached to a tmux session of a VM, streamed over a WebSocket. It reconnects while
// the VM runs, follows the size of its box, and stays mounted when hidden (no redraw on return).
// restty draws it (Ghostty's core on the GPU), xterm.js when restty cannot run.
import { memo, type Ref, useEffect, useImperativeHandle, useRef } from "react";
import { useToast } from "../../../components/Toast";
import { openView, type TerminalView } from "./engine";
import { PtySocket } from "./socket";

export type TerminalHandle = { paste: (text: string) => void; focus: () => void };

type Props = { id: string; session: string; live: boolean; visible: boolean; ref?: Ref<TerminalHandle> };

// Memoized: the machine view re-renders every second with fresh numbers; the terminal has
// nothing to redraw for them.
export const VmTerminal = memo(function VmTerminal({ id, session, live, visible, ref }: Props) {
  const el = useRef<HTMLDivElement>(null);
  const view = useRef<TerminalView | null>(null);
  const say = useToast();
  const liveRef = useRef(live);
  liveRef.current = live;
  const visibleRef = useRef(visible);
  visibleRef.current = visible;

  useImperativeHandle(ref, () => ({
    paste: (text) => view.current?.paste(text),
    focus: () => view.current?.focus(),
  }));

  useEffect(() => {
    const box = el.current;
    if (!box || !live) return;
    const socket = new PtySocket({ id, session, live: () => liveRef.current });
    let gone = false;
    openView(box, {
      socket,
      onCopied: (ok) => say(ok ? "Copied to the clipboard" : "The browser blocked the clipboard", ok ? "ok" : "err"),
    }).then((v) => {
      if (gone) return v.dispose();
      view.current = v;
      // In development, the console can reach each terminal (window.__terms.claude…).
      if (import.meta.env.DEV) {
        // oxlint-disable-next-line no-underscore-dangle -- a name no page code would ever use
        const w = window as unknown as { __terms?: Record<string, unknown> };
        // oxlint-disable-next-line no-underscore-dangle -- the same name
        w.__terms = { ...w.__terms, [session]: { view: v, socket } };
      }
      // Opened in view (the page was loaded on this machine, or the Shell just chosen): it takes
      // the keyboard now, so the first keys typed are not lost, and again once it is drawn.
      if (visibleRef.current) {
        v.focus();
        requestAnimationFrame(() => v.focus());
      }
    });
    return () => {
      gone = true;
      socket.close();
      view.current?.dispose();
      view.current = null;
    };
  }, [id, session, live, say]);

  // Back in view: take the keyboard at once, and the box's size once the browser has shown it.
  useEffect(() => {
    if (!visible) return;
    view.current?.focus();
    const frame = requestAnimationFrame(() => {
      view.current?.refit();
      view.current?.focus();
    });
    return () => cancelAnimationFrame(frame);
  }, [visible]);

  // Hidden, it keeps its size (invisible, out of reach of clicks, keys and screen readers). Shrunk
  // to nothing (display: none), restty would fit the screen to one column and lose what it shows,
  // and tmux, whose size never changed, would not draw it again on return.
  return <div ref={el} className={visible ? "term" : "term away"} inert={!visible} aria-hidden={!visible} />;
});
