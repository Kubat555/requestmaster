import { describe, expect, it } from "vitest";
import indexHtml from "../../index.html?raw";

/** The source of every component of the window, by its path. */
const components = import.meta.glob<string>("../**/*.svelte", { query: "?raw", import: "default", eager: true });

// The policy of the window (tauri.conf.json) allows neither inline styles nor inline scripts.
describe("strict CSP", () => {
  it("finds the components", () => {
    expect(Object.keys(components).length).toBeGreaterThan(0);
  });

  it("no component has a static style attribute", () => {
    // Under `style-src 'self'` the attribute is silently dropped; the `style:` directives set properties by script.
    const offenders = Object.entries(components)
      .filter(([, source]) => source.includes('style="'))
      .map(([path]) => path);
    expect(offenders).toEqual([]);
  });

  it("no component inserts raw HTML", () => {
    const offenders = Object.entries(components)
      .filter(([, source]) => source.includes("{@html"))
      .map(([path]) => path);
    expect(offenders).toEqual([]);
  });

  it("index.html has the style tag that carries the nonce of Tauri", () => {
    expect(indexHtml).toMatch(/<style>[^<]*\{[^<]*\}\s*<\/style>/);
  });
});
