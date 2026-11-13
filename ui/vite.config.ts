import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Built output is embedded into the Tauri binary (`frontendDist: ../ui/dist`); no dev server is used.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  build: {
    outDir: "dist",
    // WebView2 is Chromium-based and evergreen.
    target: "chrome105",
  },
});
