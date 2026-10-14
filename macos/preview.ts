import { headlessPreview, type NativeHeadlessPreview } from "./preview-screen";

const previews = new Map<string, NativeHeadlessPreview>();
const post = (body: Record<string, unknown>) => (window as any).webkit.messageHandlers.nativePreview.postMessage(body);

(window as any).nativePreview = {
  create: async (id: string) => {
    previews.get(id)?.dispose();
    previews.set(id, await headlessPreview());
    return true;
  },
  remove: (id: string) => { previews.get(id)?.dispose(); previews.delete(id); },
  frame: (id: string, bytes: string) => {
    const preview = previews.get(id);
    if (!preview) throw new Error("The preview parser was closed.");
    preview.write(Uint8Array.from(atob(bytes), (byte) => byte.charCodeAt(0)));
    return preview.screen();
  },
  stats: () => ({
    terminals: previews.size,
    activeWasmInstances: new Set([...previews.values()].map((p) => p.terminal.wasm)).size,
    nonblank: [...previews.values()].filter((p) => p.screen().text.trim().length > 0).length,
  }),
};

headlessPreview().then((seed) => { seed.dispose(); post({ type: "ready" }); })
  .catch((error) => post({ type: "error", message: error.message }));
