import { defineConfig } from "vite";

// frontend dev-server contract: frontend served at dev port 5173 in browser-only
// mode, then re-served by Tauri inside the desktop shell. Aggressive
// HMR is essential: animation tuning should hot-reload quickly without
// losing simulation state.
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
