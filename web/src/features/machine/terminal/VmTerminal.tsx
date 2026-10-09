// A terminal attached to a tmux session of a VM, streamed over a WebSocket. It reconnects while
// the VM runs, follows the size of its box, and stays mounted when hidden (no redraw on return).
import type { Terminal } from "@xterm/xterm";
import { type Ref, useEffect, useImperativeHandle, useRef } from "react";
import { useToast } from "../../../components/Toast";
import { copySelections, openXterm } from "./xterm";

export type TerminalHandle = { paste: (text: string) => void; focus: () => void };

type Props = { id: string; session: "claude" | "shell"; live: boolean; visible: boolean; ref?: Ref<TerminalHandle> };

export function VmTerminal({ id, session, live, visible, ref }: Props) {
  const el = useRef<HTMLDivElement>(null);
  const term = useRef<{ xterm: Terminal; resize: () => void } | null>(null);
  const say = useToast();
  const liveRef = useRef(live);
  liveRef.current = live;

  useImperativeHandle(ref, () => ({
    paste: (text) => {
      term.current?.xterm.paste(text);
      term.current?.xterm.focus();
    },
    focus: () => term.current?.xterm.focus(),
  }));

  useEffect(() => {
    const box = el.current;
    if (!box || !live) return;
    const { xterm, fit: fitter } = openXterm(box);
    const resize = () => {
      if (!box.offsetWidth) return;
      const d = fitter.proposeDimensions();
      if (d && (d.cols !== xterm.cols || d.rows !== xterm.rows)) xterm.resize(d.cols, d.rows);
    };
    term.current = { xterm, resize };
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

    return () => {
      closed = true;
      clearTimeout(retry);
      cancelAnimationFrame(frame);
      observer.disconnect();
      input.dispose();
      resized.dispose();
      ws?.close();
      xterm.dispose();
      term.current = null;
    };
  }, [id, session, live, say]);

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
}
