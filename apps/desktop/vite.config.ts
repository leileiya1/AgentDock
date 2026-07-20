import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

// Tauri desktop frontend. Dev server runs on a fixed port so `tauri dev`
// (build.devUrl in tauri.conf.json) can attach to it. Production build lands in
// ../desktop/dist; see README note about the frontendDist wiring in src-tauri.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  // Prevent Vite from obscuring rust errors in the tauri dev console.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    outDir: "dist",
    target: "es2021",
    sourcemap: false,
    // Monaco's language workers are large; keep them in their own chunks.
    chunkSizeWarningLimit: 2000,
    rollupOptions: {
      output: {
        // Split heavy vendors out of the main bundle. Monaco is intentionally
        // left alone — it's reached only via the lazy MonacoDiff import and
        // already lands in its own on-demand chunk.
        manualChunks(id) {
          if (!id.includes("node_modules")) return;
          if (id.includes("monaco-editor")) return;
          if (id.includes("@radix-ui")) return "vendor-radix";
          if (id.includes("@tanstack")) return "vendor-query";
          if (
            id.includes("/react-router") ||
            id.includes("/react-dom/") ||
            id.includes("/react/") ||
            id.includes("/scheduler/")
          )
            return "vendor-react";
          if (
            id.includes("/motion/") ||
            id.includes("/motion-dom/") ||
            id.includes("/motion-utils/") ||
            id.includes("framer-motion")
          )
            return "vendor-motion";
        },
      },
    },
  },
});
