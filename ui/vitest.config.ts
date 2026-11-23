import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";

// Separate from vite.config.ts, so the plugins are listed again. Without `svelteTesting` the server build
// of Svelte is resolved and `mount` fails.
export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  // The table of the script API lies outside this folder, in the core; scriptCompletions.test.ts reads it as text
  // to compare with the list of the window. Without the entry the import is refused ("Denied ID").
  server: { fs: { allow: [".", "../crates/rqm-script/prelude/src"] } },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/test-setup.ts"],
    // Vitest replaces every CSS import with an empty one, `?raw` included; a stylesheet imported as text
    // (contrast.test.ts reads tokens.css) has to stay as it is.
    css: { include: [/\.css\?raw$/] },
  },
});
