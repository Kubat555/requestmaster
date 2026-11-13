import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";

// Separate from vite.config.ts, so the plugins are listed again. Without `svelteTesting` the server build
// of Svelte is resolved and `mount` fails.
export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/test-setup.ts"],
    // Vitest replaces every CSS import with an empty one, `?raw` included; a stylesheet imported as text
    // (contrast.test.ts reads tokens.css) has to stay as it is.
    css: { include: [/\.css\?raw$/] },
  },
});
