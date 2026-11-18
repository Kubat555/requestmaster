// Bundles the prelude into `built/`. Run with `npm run build` inside this folder; `cargo` never calls Node.
//
// The first line of every built file is `// rqm-prelude-hash: <16 hex digits>` — FNV-1a 64 over the sources.
// The Rust test `built_files_match_their_sources` computes the same sum, so a source that was changed
// and not rebuilt is caught without Node.
import { build } from "esbuild";
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";

const BUNDLES = [
  { entry: "src/index.js", out: "built/prelude.js" },
  // chai 4.4.1 and the assertions of a response; loaded into a context on first use.
  { entry: "src/chai-entry.js", out: "built/chai.js" },
];

/** The files the sum covers: the build itself, then everything in `src/` by name. */
function hashedFiles() {
  const sources = readdirSync("src", { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => `src/${entry.name}`)
    .sort();
  return ["build.mjs", "package.json", "package-lock.json", ...sources];
}

/** For every file: its name, 0x0A, its bytes without 0x0D (Windows line ends do not change the sum), 0x0A. */
function sourcesHash() {
  const mask = (1n << 64n) - 1n;
  let hash = 0xcbf29ce484222325n;
  const feed = (byte) => {
    hash = ((hash ^ BigInt(byte)) * 0x100000001b3n) & mask;
  };
  for (const name of hashedFiles()) {
    for (const byte of Buffer.from(name, "utf8")) feed(byte);
    feed(0x0a);
    for (const byte of readFileSync(name)) if (byte !== 0x0d) feed(byte);
    feed(0x0a);
  }
  return hash.toString(16).padStart(16, "0");
}

const hash = sourcesHash();
mkdirSync("built", { recursive: true });
for (const bundle of BUNDLES) {
  const result = await build({
    entryPoints: [bundle.entry],
    bundle: true,
    format: "iife",
    platform: "neutral",
    mainFields: ["main", "module"],
    target: "es2020",
    minify: true,
    legalComments: "none",
    write: false,
  });
  writeFileSync(bundle.out, `// rqm-prelude-hash: ${hash}\n${result.outputFiles[0].text}`);
}
