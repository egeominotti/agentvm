import { toast } from "../ui/toast.js";

/** Text selected in tmux arrives as OSC 52 (base64): put it on the Mac's clipboard. */
export function copySelectionsToClipboard(xterm, readOnly) {
  xterm.parser.registerOscHandler(52, data => {
    const b64 = data.slice(data.indexOf(";") + 1);
    if (readOnly || !b64 || b64 === "?") return true;
    try {
      const text = new TextDecoder().decode(Uint8Array.from(atob(b64), c => c.charCodeAt(0)));
      navigator.clipboard.writeText(text).then(() => toast("Copied to the clipboard"), () => toast("The browser blocked the clipboard", "err"));
    } catch {}
    return true;
  });
}
