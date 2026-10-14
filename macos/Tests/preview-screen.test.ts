import { describe, expect, test } from "bun:test";
import { headlessPreview } from "../preview-screen";

describe("native preview through the actual Ghostty core", () => {
  test("a terminal redraw replaces erased text", async () => {
    const preview = await headlessPreview();
    try {
      preview.write(new TextEncoder().encode("old secret\rnew\x1b[K"));
      expect(preview.screen().text).toBe("new");
      expect(preview.screen().rows).toBe(16);
      expect(preview.screen().cols).toBe(60);
    } finally { preview.dispose(); }
  });

  test("UTF-8 split across frames preserves wide characters and graphemes", async () => {
    const preview = await headlessPreview();
    try {
      const bytes = new TextEncoder().encode("😀界e\u0301");
      preview.write(bytes.subarray(0, 2));
      preview.write(bytes.subarray(2, 5));
      preview.write(bytes.subarray(5));
      expect(preview.screen().text).toBe("😀界e\u0301");
    } finally { preview.dispose(); }
  });

  test("shared WASM terminals retain independent screen state", async () => {
    const first = await headlessPreview();
    const second = await headlessPreview();
    try {
      first.write(new TextEncoder().encode("first"));
      second.write(new TextEncoder().encode("second"));
      expect(first.terminal.wasm === second.terminal.wasm).toBe(true);
      expect(first.terminal.handle).not.toBe(second.terminal.handle);
      expect(first.screen().text).toBe("first");
      expect(second.screen().text).toBe("second");
    } finally { first.dispose(); second.dispose(); }
  });

  test("invisible terminal text never appears in a native preview", async () => {
    const preview = await headlessPreview();
    try {
      preview.write(new TextEncoder().encode("visible\x1b[8mhidden\x1b[0m"));
      expect(preview.screen().text).toBe("visible");
    } finally { preview.dispose(); }
  });
});
