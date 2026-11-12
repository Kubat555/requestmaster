// Bundles entry.js into dist/prelude.js (IIFE, platform neutral) for the QuickJS probe.
import { build } from "esbuild";
import { statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const outfile = join(here, "dist", "prelude.js");

await build({
  entryPoints: [join(here, "entry.js")],
  bundle: true,
  format: "iife",
  platform: "neutral",
  mainFields: ["main", "module"],
  target: "es2020",
  outfile,
  logLevel: "info",
});

console.log(`prelude: ${outfile} (${statSync(outfile).size} bytes)`);
