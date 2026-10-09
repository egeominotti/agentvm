// A live, read-only preview of a machine's Claude session, drawn on the GPU like every terminal. It keeps the session's real size and
// is scaled down to its box (attaching smaller would crop it), streams only while on screen and
// while the page is visible, and parses at most 4 times a second whatever the VM prints.
import { useEffect, useRef, useState } from "react";
import { monoFont, openXterm } from "../machine/terminal/xterm";

const COLS = 140;
const ROWS = 42;

export function Preview({ id }: { id: string }) {
  const box = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const [fontReady, setFontReady] = useState(false);
  useEffect(() => {
    monoFont().then(() => setFontReady(true));
  }, []);

  useEffect(() => {
    const outer = box.current;
    const el = inner.current;
    if (!outer || !el || !fontReady) return;
    const { xterm, close } = openXterm(el, { fontSize: 12, readOnly: true });
    xterm.resize(COLS, ROWS);
    const scale = () => {
      const screen = el.querySelector<HTMLElement>(".xterm-screen");
      if (!screen?.offsetWidth) return;
      const k = Math.min(outer.clientWidth / screen.offsetWidth, outer.clientHeight / screen.offsetHeight, 1);
      el.style.transform = `scale(${k})`;
    };
    const sized = new ResizeObserver(scale);
    sized.observe(outer);

    let ws: WebSocket | null = null;
    let onScreen = true;
    let gone = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let chunks: Uint8Array[] = [];
    let flush: ReturnType<typeof setTimeout> | undefined;
    const write = (data: ArrayBuffer) => {
      chunks.push(new Uint8Array(data));
      flush ??= setTimeout(() => {
        flush = undefined;
        for (const c of chunks) xterm.write(c);
        chunks = [];
      }, 250);
    };
    const connect = () => {
      if (ws || gone || !onScreen || document.hidden) return;
      const sock = new WebSocket(
        `ws://${location.host}/api/tasks/${id}/pty?session=claude&cols=${COLS}&rows=${ROWS}&view=true`,
      );
      sock.binaryType = "arraybuffer";
      sock.onopen = scale;
      sock.onmessage = (e) => write(e.data as ArrayBuffer);
      sock.onclose = () => {
        if (ws !== sock) return;
        ws = null;
        retry = setTimeout(connect, 1500);
      };
      ws = sock;
    };
    // tmux redraws the whole screen on attach: pausing loses nothing.
    const pause = () => {
      const sock = ws;
      ws = null;
      sock?.close();
    };
    const follow = () => (onScreen && !document.hidden ? connect() : pause());
    const seen = new IntersectionObserver(([e]) => {
      onScreen = !!e?.isIntersecting;
      follow();
    });
    seen.observe(outer);
    document.addEventListener("visibilitychange", follow);
    connect();

    return () => {
      gone = true;
      clearTimeout(retry);
      clearTimeout(flush);
      seen.disconnect();
      sized.disconnect();
      document.removeEventListener("visibilitychange", follow);
      pause();
      close();
    };
  }, [id, fontReady]);

  return (
    <div ref={box} className="preview" aria-hidden="true">
      <div ref={inner} className="preview-term" />
    </div>
  );
}
