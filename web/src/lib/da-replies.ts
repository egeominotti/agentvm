// Device Attributes replies, corrected. A program (tmux, on attach) asks the terminal what it is
// with `ESC [ c` (primary) and `ESC [ > c` (secondary). restty answers both in the primary form
// (`ESC [ ? 1 ; 2 c`): tmux does not expect it for the secondary one and passes it on to the shell,
// which shows "1;2c" as if typed. The queries are followed in the output, in order, and the
// answer to a secondary one gets the secondary form.

// biome-ignore lint/suspicious/noControlCharactersInRegex: ESC is what a terminal query starts with
const QUERY = /\x1b\[(>)?0?c/g;
const PRIMARY_REPLY = "\x1b[?1;2c";
/** VT220-like, firmware 10: what a secondary query expects in form. */
const SECONDARY_REPLY = "\x1b[>1;10;0c";

export class DeviceAttributes {
  private asked: ("primary" | "secondary")[] = [];
  /** A query cut at the end of a chunk: kept for the next one. */
  private tail = "";

  /** Output from the program, before the terminal answers it. */
  output(text: string) {
    const all = this.tail + text;
    for (const m of all.matchAll(QUERY)) this.asked.push(m[1] ? "secondary" : "primary");
    const cut = all.lastIndexOf("\x1b");
    this.tail = cut >= 0 && all.length - cut < 5 && !QUERY.test(all.slice(cut)) ? all.slice(cut) : "";
    QUERY.lastIndex = 0;
    // A terminal never asked again would let the list grow: only recent queries matter.
    if (this.asked.length > 16) this.asked.splice(0, this.asked.length - 16);
  }

  /** What the terminal sends back, corrected when it answers a secondary query. */
  reply(data: string): string {
    if (data !== PRIMARY_REPLY) return data;
    return this.asked.shift() === "secondary" ? SECONDARY_REPLY : data;
  }
}
