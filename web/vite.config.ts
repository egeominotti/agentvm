/// <reference types="vitest/config" />
// The dashboard: built into dist/ and embedded in the server binary (server/build.rs).
// `bun run dev` serves it with hot reload, proxying the API to a running agentvm-server
// (AGENTVM_API, http://127.0.0.1:7777 by default).
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Set by Bun (or Node) when the config runs; the app itself never sees it.
declare const process: { env: Record<string, string | undefined> };
const API = process.env.AGENTVM_API ?? "http://127.0.0.1:7777";

export default defineConfig({
  base: "/",
  plugins: [react()],
  build: { outDir: "dist", sourcemap: false, chunkSizeWarningLimit: 1500 },
  server: {
    proxy: {
      "/api": {
        target: API,
        ws: true,
        changeOrigin: true,
        // The server only answers its own origin (no cross-site WebSocket to a terminal): the dev
        // server speaks for the page, so it says it comes from the server itself.
        headers: { origin: API },
      },
    },
  },
  test: { environment: "jsdom" },
});
