// Runs one shell command inside a running VM and prints its output: how an agent checks what is
// in a machine without a browser. Uses its own shell (Shell 9), so the user's shells are untouched.
//
//   bun scripts/vm-run.ts <port> <vm-id> '<command>'      e.g. bun scripts/vm-run.ts 7790 01a1… 'git log -1'
//   WAIT=120000 bun scripts/vm-run.ts …                     for slow commands (default 15 s)
//   SESSION=shell bun scripts/vm-run.ts …                   VMs started before extra shells existed
const [port, id, ...words] = process.argv.slice(2);
if (!port || !id || !words.length) {
  console.error("usage: bun scripts/vm-run.ts <port> <vm-id> '<command>'");
  process.exit(2);
}
const session = process.env.SESSION ?? "shell-9";
const origin = `http://127.0.0.1:${port}`;
const ws = new WebSocket(`ws://127.0.0.1:${port}/api/tasks/${id}/pty?session=${session}&cols=200&rows=50`, {
  headers: { Origin: origin },
} as unknown as string[]);
ws.binaryType = "arraybuffer";
const mark = `AGENTVM${Date.now() % 1_000_000}`;
let out = "";
// Aliases off first, on a line of their own (a line's aliases are expanded as it is read): the
// VM's shell maps cat to bat and ls to eza, and the output must be the plain one.
ws.onopen = () =>
  ws.send(new TextEncoder().encode(`unalias -a 2>/dev/null\recho ${mark}-A; ${words.join(" ")}; echo ${mark}-B $?\r`));
ws.onmessage = (e) => {
  out += new TextDecoder().decode(e.data as ArrayBuffer);
  const done = out.match(new RegExp(`\\n${mark}-A\\r?\\n([\\s\\S]*?)${mark}-B (\\d+)`));
  if (!done) return;
  console.log(done[1].replace(/\x1b\[[0-9;?]*[A-Za-z]|\x1b\([AB0]/g, "").replace(/\r/g, "").trimEnd());
  process.exit(Number(done[2]));
};
ws.onclose = () => {
  console.error(`the terminal closed (is ${id} running on :${port}?)`);
  process.exit(1);
};
setTimeout(() => {
  console.error(`no answer in time; last output:\n${out.slice(-800)}`);
  process.exit(124);
}, Number(process.env.WAIT ?? 15000));
