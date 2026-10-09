import { expect, it } from "vitest";
import { parseDiff } from "./diff";

it("splits a unified diff into files with their counts", () => {
  const text = [
    "diff --git a/calc.py b/calc.py",
    "index 1..2 100644",
    "--- a/calc.py",
    "+++ b/calc.py",
    "@@ -1,2 +1,2 @@",
    " def add(a, b):",
    "-    return a - b",
    "+    return a + b",
    "diff --git a/new.txt b/new.txt",
    "new file mode 100644",
    "+hello",
  ].join("\n");
  const files = parseDiff(text);
  expect(files.map((f) => [f.path, f.add, f.del])).toEqual([
    ["calc.py", 1, 1],
    ["new.txt", 1, 0],
  ]);
  expect(files[0]?.lines).toEqual(["@@ -1,2 +1,2 @@", " def add(a, b):", "-    return a - b", "+    return a + b"]);
});

it("an empty diff has no files", () => {
  expect(parseDiff("")).toEqual([]);
});
