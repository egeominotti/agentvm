// A terminal drawn by xterm.js (WebGL): the fallback when restty cannot start (no WebGPU or
// WebGL2, or it failed). Same socket, same colors, same clipboard as the restty view.
import type { TerminalView, ViewOptions } from "./engine";
import { copySelections, openXterm } from "./xterm";

export function openXtermView(el: HTMLElement, opts: ViewOptions): TerminalView {
  const { socket, readOnly = false, fontSize = 13, fixed, onCopied } = opts;
  const { xterm, fit, close } = openXterm(el, { fontSize, readOnly });
  el.dataset.engine = "xterm";
  if (onCopied) copySelections(xterm, onCopied);
  socket.onOutput((bytes) => xterm.write(bytes));
  const input = xterm.onData((d) => {
    if (!readOnly) socket.send(d);
  });
  const resized = xterm.onResize(({ cols, rows }) => {
    if (!fixed) socket.resize(cols, rows);
  });
  const refit = () => {
    if (fixed) return;
    if (!el.offsetWidth) return;
    const d = fit.proposeDimensions();
    if (d && (d.cols !== xterm.cols || d.rows !== xterm.rows)) xterm.resize(d.cols, d.rows);
  };
  if (fixed) xterm.resize(fixed.cols, fixed.rows);
  else {
    refit();
    if (xterm.cols < 20) xterm.resize(100, 30);
  }
  let frame = 0;
  const observer = new ResizeObserver(() => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(refit);
  });
  if (!fixed) observer.observe(el);
  socket.connect(xterm.cols, xterm.rows);
  return {
    engine: "xterm",
    focus: () => xterm.focus(),
    paste: (text) => {
      xterm.paste(text);
      xterm.focus();
    },
    refit,
    dispose: () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      input.dispose();
      resized.dispose();
      close();
    },
  };
}
