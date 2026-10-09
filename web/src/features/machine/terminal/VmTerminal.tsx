// A terminal attached to a tmux session of a VM, streamed over a WebSocket. It reconnects while
// the VM runs, follows the size of its box, and stays mounted when hidden (no redraw on return).
import type { Terminal } from "@xterm/xterm";
import { memo, type Ref, useEffect, useImperativeHandle, useRef, useState } from "react";
import { useToast } from "../../../components/Toast";
import { copySelections, monoFont, openXterm } from "./xterm";

export type TerminalHandle = { paste: (text: string) => void; focus: () => void };

type Props = { id: string; session: "claude" | "shell"; live: boolean; visible: boolean; ref?: Ref<TerminalHandle> };

// Memoized: the machine view re-renders every second with fresh numbers; the terminal has
// nothing to redraw for them.
export const VmTerminal = memo(function VmTerminal({ id, session, live, visible, ref }: Props) {
  const el = useRef<HTMLDivElement>(null);
  const term = useRef<{ xterm: Terminal; resize: () => void } | null>(null);
  const say = useToast();
  const liveRef = useRef(live);
  liveRef.current = live;
  const visibleRef = useRef(visible);
  visibleRef.current = visible;
  const [fontReady, setFontReady] = useState(false);
  useEffect(() => {
    monoFont().then(() => setFontReady(true));
  }, []);

  useImperativeHandle(ref, () => ({
    paste: (text) => {
      term.current?.xterm.paste(text);
      term.current?.xterm.focus();
    },
    focus: () => term.current?.xterm.focus(),
  }));

  useEffect(() => {
    const box = el.current;
    if (!box || !live || !fontReady) return;
    const { xterm, fit: fitter, close } = openXterm(box);
    const resize = () => {
      if (!box.offsetWidth) return;
      const d = fitter.proposeDimensions();
      if (d && (d.cols !== xterm.cols || d.rows !== xterm.rows)) xterm.resize(d.cols, d.rows);
    };
    term.current = { xterm, resize };
    // In development, the console can reach each terminal (window.__terms.claude…).
    if (import.meta.env.DEV) {
      const w = window as unknown as { __terms?: Record<string, unknown> };
      w.__terms = { ...w.__terms, [session]: { xterm, ws: () => ws } };
    }
    copySelections(xterm, (ok) =>
      say(ok ? "Copied to the clipboard" : "The browser blocked the clipboard", ok ? "ok" : "err"),
    );

    let ws: WebSocket | null = null;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let closed = false;
    const enc = new TextEncoder();
    // Keys typed while the socket opens are kept and sent once it is open, not lost.
    let pending: Uint8Array[] = [];
    const sendSize = () => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ cols: xterm.cols, rows: xterm.rows }));
    };
    const connect = () => {
      resize();
      if (xterm.cols < 20) xterm.resize(100, 30);
      const url = `ws://${location.host}/api/tasks/${id}/pty?session=${session}&cols=${xterm.cols}&rows=${xterm.rows}`;
      const sock = new WebSocket(url);
      sock.binaryType = "arraybuffer";
      // Sizes sent while connecting are lost: send the real one once the socket is open.
      sock.onopen = () => {
        resize();
        sendSize();
        for (const p of pending) sock.send(p);
        pending = [];
      };
      sock.onmessage = (e) => xterm.write(new Uint8Array(e.data as ArrayBuffer));
      sock.onclose = () => {
        if (ws !== sock) return;
        ws = null;
        if (!closed && liveRef.current) retry = setTimeout(connect, 900);
      };
      ws = sock;
    };
    const input = xterm.onData((d) => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(enc.encode(d));
      else if (pending.length < 256) pending.push(enc.encode(d));
    });
    const resized = xterm.onResize(sendSize);
    let frame = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(resize);
    });
    observer.observe(box);
    connect();
    // Opened in view (the page was loaded on this machine): it takes the keyboard at once.
    if (visibleRef.current) requestAnimationFrame(() => xterm.focus());

    return () => {
      closed = true;
      clearTimeout(retry);
      cancelAnimationFrame(frame);
      observer.disconnect();
      input.dispose();
      resized.dispose();
      ws?.close();
      close();
      term.current = null;
    };
  }, [id, session, live, say, fontReady]);

  // Back in view: take the box's size and the keyboard (once the browser has shown it).
  useEffect(() => {
    if (!visible) return;
    const frame = requestAnimationFrame(() => {
      term.current?.resize();
      term.current?.xterm.focus();
    });
    return () => cancelAnimationFrame(frame);
  }, [visible]);

  return <div ref={el} className="term" hidden={!visible} />;
});
