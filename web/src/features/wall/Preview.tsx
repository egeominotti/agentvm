// A live, read-only preview of a machine's Claude session. Ghostty's terminal core (restty's
// headless build, one WebAssembly runtime shared by every preview) reads the stream, and the
// browser draws its lines as text (screen.ts): nothing per preview on the GPU, no font parsed per
// preview, so a wall of many machines opens at once and scrolls freely. Off screen, or while the
// page is hidden, it drops its stream and keeps what it showed; tmux redraws it all on return.
import { useEffect, useRef } from "react";
import type { ResttyHeadlessTerminal } from "restty/headless";
import { pageTheme, terminalTheme } from "../machine/terminal/palette";
import { PtySocket } from "../machine/terminal/socket";
import { type Colors, extent, hex, lineHtml, themeColors } from "./screen";

// Larger than any window a machine page gives the session: the whole of it is seen, and the
// preview is cut to what is drawn on.
const COLS = 200;
const ROWS = 60;
// The text is redrawn at most this often: a preview is glanced at, not read live.
const FRAME_MS = 200;

let runtime: Promise<() => ResttyHeadlessTerminal> | null = null;

/** A new headless terminal; the WebAssembly runtime is decoded and compiled only once. */
function headless(): Promise<ResttyHeadlessTerminal> {
  runtime ??= import("restty/headless").then(async ({ createHeadlessTerminal, ResttyHeadlessTerminal }) => {
    const options = { cols: COLS, rows: ROWS, replay: false as const, maxScrollbackBytes: 0 };
    const { wasm } = await createHeadlessTerminal(options);
    return () => new ResttyHeadlessTerminal(wasm, options);
  });
  return runtime.then((create) => create());
}

function paint(term: ResttyHeadlessTerminal, el: HTMLElement, c: Colors) {
  el.style.color = hex(c.fg);
  term.wasm.setDefaultColors(term.handle, c.fg, c.bg, c.cursor);
  term.wasm.setPalette(term.handle, c.palette, c.palette.length / 3);
  term.renderUpdate();
}

export function Preview({ id }: { id: string }) {
  const box = useRef<HTMLDivElement>(null);
  const screen = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const outer = box.current;
    const el = screen.current;
    if (!outer || !el) return;
    const socket = new PtySocket({ id, session: "claude", view: true, live: () => true });
    socket.resize(COLS, ROWS);
    const decoder = new TextDecoder();
    const lines: string[] = [];
    const rows = Array.from({ length: ROWS }, () => el.appendChild(document.createElement("div")));
    let colors = themeColors(terminalTheme(pageTheme()));
    let term: ResttyHeadlessTerminal | null = null;
    let early: string[] = [];
    let gone = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let drawnAt = 0;

    const fit = () => {
      if (!el.offsetWidth) return;
      const k = Math.min(outer.clientWidth / el.offsetWidth, outer.clientHeight / el.offsetHeight, 1);
      el.style.transform = `scale(${k})`;
    };
    const draw = () => {
      timer = undefined;
      drawnAt = performance.now();
      const cells = term?.getRenderState();
      if (!cells) return;
      for (let r = 0; r < ROWS; r += 1) {
        const html = lineHtml(cells, r, colors);
        if (lines[r] !== html) {
          lines[r] = html;
          rows[r]!.innerHTML = html;
        }
      }
      const size = extent(cells, colors);
      el.style.setProperty("--cols", String(Math.max(size.cols, 60)));
      el.style.setProperty("--rows", String(Math.max(size.rows, 16)));
    };
    const later = () => {
      timer ??= setTimeout(() => requestAnimationFrame(draw), Math.max(0, drawnAt + FRAME_MS - performance.now()));
    };

    socket.onOutput((bytes) => {
      const text = decoder.decode(bytes, { stream: true });
      if (!term) early.push(text);
      else {
        term.write(text);
        later();
      }
    });
    headless().then((t) => {
      if (gone) return t.dispose();
      term = t;
      paint(t, el, colors);
      for (const text of early) t.write(text);
      early = [];
      draw();
    });

    // Streams only while on screen (or close to it) and while the page is in view.
    let near = false;
    const follow = () => (near && !document.hidden ? socket.resume() : socket.pause());
    const seen = new IntersectionObserver(
      ([e]) => {
        near = !!e?.isIntersecting;
        follow();
      },
      { rootMargin: "200px" },
    );
    seen.observe(outer);
    document.addEventListener("visibilitychange", follow);
    const sized = new ResizeObserver(fit);
    sized.observe(outer);
    sized.observe(el);
    // Light or dark follows the page.
    const recolor = new MutationObserver(() => {
      colors = themeColors(terminalTheme(pageTheme()));
      if (term) paint(term, el, colors);
      lines.length = 0;
      draw();
    });
    recolor.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

    return () => {
      gone = true;
      clearTimeout(timer);
      seen.disconnect();
      sized.disconnect();
      recolor.disconnect();
      document.removeEventListener("visibilitychange", follow);
      socket.close();
      term?.dispose();
      el.replaceChildren();
    };
  }, [id]);

  return (
    <div ref={box} className="preview" aria-hidden="true">
      <div ref={screen} className="preview-screen" />
    </div>
  );
}
