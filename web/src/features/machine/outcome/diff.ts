// A unified diff split into files, each with its changed lines and its counts.

export type DiffFile = { path: string; add: number; del: number; lines: string[] };

const HEADER = /^(index |--- |\+\+\+ |new file mode|deleted file mode|similarity |rename )/;

export function parseDiff(text: string): DiffFile[] {
  const files: DiffFile[] = [];
  let cur: DiffFile | null = null;
  for (const line of text.split("\n")) {
    if (line.startsWith("diff --git")) {
      cur = { path: line.match(/ b\/(.+)$/)?.[1] ?? line, add: 0, del: 0, lines: [] };
      files.push(cur);
    } else if (cur && !HEADER.test(line)) {
      if (line.startsWith("+")) cur.add++;
      else if (line.startsWith("-")) cur.del++;
      cur.lines.push(line);
    }
  }
  return files;
}
