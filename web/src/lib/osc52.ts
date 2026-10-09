// Clipboard writes in a terminal's output (OSC 52: `ESC ] 52 ; c ; <base64> BEL|ST`). tmux sends
// one when text is selected with the mouse: it goes to the Mac's clipboard. Output arrives in
// chunks, so a sequence may be cut in two: the unfinished part is kept for the next chunk.

const START = "\x1b]52;";
/** A selection larger than this is not a clipboard write worth holding output for. */
const MAX_PENDING = 8 << 20;

export class Osc52Scanner {
  private pending = "";

  /** The texts written to the clipboard by `chunk` (and what came before it). */
  push(chunk: string): string[] {
    let text = this.pending + chunk;
    this.pending = "";
    const found: string[] = [];
    for (;;) {
      const start = text.indexOf(START);
      if (start < 0) break;
      const bel = text.indexOf("\x07", start);
      const st = text.indexOf("\x1b\\", start);
      const end = bel < 0 ? st : st < 0 ? bel : Math.min(bel, st);
      if (end < 0) {
        // Cut off: wait for the rest (unless it is absurdly long).
        if (text.length - start <= MAX_PENDING) this.pending = text.slice(start);
        break;
      }
      const body = text.slice(start + START.length, end);
      const data = body.slice(body.indexOf(";") + 1);
      const decoded = decode(data);
      if (decoded !== null) found.push(decoded);
      text = text.slice(end + (text[end] === "\x07" ? 1 : 2));
    }
    return found;
  }
}

function decode(b64: string): string | null {
  if (!b64 || b64 === "?" || !/^[A-Za-z0-9+/=]+$/.test(b64)) return null;
  try {
    return new TextDecoder().decode(Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)));
  } catch {
    return null;
  }
}
