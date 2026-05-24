import { defineConfig } from "vite";

// PLAN §3.1 / §14: frontend served at dev port 5173 in browser-only
// mode, then re-served by Tauri inside the desktop shell. Aggressive
// HMR is essential — Step 18 measures animations.ts hot-reload at
// <300ms (PLAN §14, §20 DoD item 5).
export default defineConfig({
  root: ".",
  publicDir: "public",
  build: {
    target: "es2022",
    outDir: "dist",
    sourcemap: true,
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    strictPort: true,
    host: "127.0.0.1",
    hmr: {
      overlay: true,
    },
  },
});
