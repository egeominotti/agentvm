/// <reference types="vitest/config" />
// The dashboard: built into dist/ and embedded in the server binary (server/build.rs).
// `bun run dev` serves it with hot reload, proxying the API to a running agentvm-server.
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  // Served at /next/ while it replaces the dashboard at / screen by screen.
  base: "/next/",
  plugins: [react()],
  build: { outDir: "dist", sourcemap: false, chunkSizeWarningLimit: 1500 },
  server: { proxy: { "/api": { target: "http://127.0.0.1:7777", ws: true } } },
  test: { environment: "jsdom" },
});
