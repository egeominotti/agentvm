// A live, read-only preview of a machine's Claude session, drawn like every terminal (restty on
// the GPU, xterm.js if it cannot run). It keeps the session's real size and is scaled down to its
// box (attaching smaller would crop it). It exists only while on screen: off screen it gives back
// its GPU resources and its stream, so a wall of many machines stays light. While the page is
// hidden it streams nothing, and it takes in what the VM prints at most 4 times a second.
import { useEffect, useRef, useState } from "react";
import { openView } from "../machine/terminal/engine";
import { PtySocket } from "../machine/terminal/socket";

const COLS = 140;
const ROWS = 42;

/** What the engine drew: xterm's screen, or restty's canvas. */
function drawn(el: HTMLElement): HTMLElement | null {
  return el.querySelector<HTMLElement>(".xterm-screen") ?? el.querySelector<HTMLElement>("canvas");
}

export function Preview({ id }: { id: string }) {
  const box = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const [onScreen, setOnScreen] = useState(false);
  useEffect(() => {
    const outer = box.current;
    if (!outer) return;
    const seen = new IntersectionObserver(([e]) => setOnScreen(!!e?.isIntersecting), { rootMargin: "200px" });
    seen.observe(outer);
    return () => seen.disconnect();
  }, []);

  useEffect(() => {
    const outer = box.current;
    const el = inner.current;
    if (!outer || !el || !onScreen) return;
    const socket = new PtySocket({ id, session: "claude", view: true, live: () => true, batchMs: 250 });
    const scale = () => {
      const screen = drawn(el);
      if (!screen?.offsetWidth) return;
      const k = Math.min(outer.clientWidth / screen.offsetWidth, outer.clientHeight / screen.offsetHeight, 1);
      el.style.transform = `scale(${k})`;
    };
    const sized = new ResizeObserver(scale);
    sized.observe(outer);
    socket.onOpen(scale);
    let gone = false;
    let dispose = () => {};
    openView(el, { socket, readOnly: true, fontSize: 12, fixed: { cols: COLS, rows: ROWS } }).then((v) => {
      if (gone) return v.dispose();
      dispose = v.dispose;
      requestAnimationFrame(scale);
    });
    // tmux redraws the whole screen on attach: pausing while hidden loses nothing.
    const follow = () => (document.hidden ? socket.pause() : socket.resume());
    document.addEventListener("visibilitychange", follow);
    if (document.hidden) socket.pause();

    return () => {
      gone = true;
      sized.disconnect();
      document.removeEventListener("visibilitychange", follow);
      socket.close();
      dispose();
    };
  }, [id, onScreen]);

  return (
    <div ref={box} className="preview" aria-hidden="true">
      <div ref={inner} className="preview-term" />
    </div>
  );
}
