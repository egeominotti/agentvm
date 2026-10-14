import { createHeadlessTerminal, ResttyHeadlessTerminal } from "../web/node_modules/restty/dist/headless.js";

export type NativePreviewScreen = { text: string; cols: number; rows: number };

export class NativeHeadlessPreview {
  private decoder = new TextDecoder();
  constructor(readonly terminal: ResttyHeadlessTerminal) {}
  write(bytes: Uint8Array) { this.terminal.write(this.decoder.decode(bytes, { stream: true })); }
  screen(): NativePreviewScreen {
    const state = this.terminal.getRenderState();
    if (!state) return { text: "", cols: 60, rows: 16 };
    const lines: string[] = [];
    let extent = 0;
    for (let row = 0; row < state.rows; row++) {
      let line = "";
      for (let col = 0; col < state.cols; col++) {
        const index = row * state.cols + col;
        if (state.wide?.[index] === 2) continue;
        const cp = state.codepoints?.[index] ?? 0;
        let text = cp > 32 && !((state.styleFlags?.[index] ?? 0) & 32) ? String.fromCodePoint(cp) : " ";
        const count = state.graphemeLen?.[index] ?? 0;
        if (text !== " " && count && state.graphemeBuffer && state.graphemeOffset) {
          const offset = state.graphemeOffset[index]!;
          text += String.fromCodePoint(...state.graphemeBuffer.subarray(offset, offset + count));
        }
        if (text !== " ") extent = Math.max(extent, col + (state.wide?.[index + 1] === 2 ? 2 : 1));
        line += text;
      }
      lines.push(line.replace(/ +$/u, ""));
    }
    while (lines.length && lines.at(-1) === "") lines.pop();
    return { text: lines.join("\n"), cols: Math.max(60, extent), rows: Math.max(16, lines.length) };
  }
  dispose() { this.terminal.dispose(); }
}

const options = { cols: 200, rows: 60, replay: false as const, maxScrollbackBytes: 0 };
let runtime: Promise<ResttyHeadlessTerminal["wasm"]> | null = null;

export async function headlessPreview(): Promise<NativeHeadlessPreview> {
  runtime ??= createHeadlessTerminal(options).then((seed) => {
    const wasm = seed.wasm;
    seed.dispose();
    return wasm;
  });
  return new NativeHeadlessPreview(new ResttyHeadlessTerminal(await runtime, options));
}
