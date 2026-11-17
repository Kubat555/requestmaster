import { describe, expect, it } from "vitest";
import base from "./base.css?raw";
import tokens from "./tokens.css?raw";

/** The source of every component of the window, by its path. */
const components = import.meta.glob<string>("../**/*.svelte", { query: "?raw", import: "default", eager: true });
/** The scripts of the window, tests aside. */
const scripts = import.meta.glob<string>(["../**/*.ts", "!../**/*.test.ts"], { query: "?raw", import: "default", eager: true });

/** The stylesheet of a component: what stands in its `<style>`. */
function styleOf(source: string): string {
  return /<style>([\s\S]*?)<\/style>/.exec(source)?.[1] ?? "";
}

/** Every stylesheet of the window, by its path. */
const sheets: Record<string, string> = {
  "tokens.css": tokens,
  "base.css": base,
  ...Object.fromEntries(Object.entries(components).map(([path, source]) => [path, styleOf(source)])),
};

/** The rules of a stylesheet that are not keyframes: the selector and the declarations of each. */
function rulesOf(css: string): { selector: string; body: string }[] {
  const rules: { selector: string; body: string }[] = [];
  const text = css.replace(/\/\*[\s\S]*?\*\//g, "");
  let at = 0;
  while (at < text.length) {
    const open = text.indexOf("{", at);
    if (open === -1) break;
    let depth = 1;
    let close = open + 1;
    for (; close < text.length && depth > 0; close++) {
      if (text[close] === "{") depth++;
      else if (text[close] === "}") depth--;
    }
    const selector = text.slice(at, open).trim().replace(/\s+/g, " ");
    if (!selector.startsWith("@keyframes")) rules.push({ selector, body: text.slice(open + 1, close - 1) });
    at = close;
  }
  return rules;
}

const OFF = '[data-motion="off"]';
const RICH = '[data-motion="rich"]';
/** The tokens that exist for the rich level alone. */
const RICH_TOKENS = ["--dur-5", "--dur-6", "--ease-spring", "--stagger", "--dur-glow", "--glow", "--blur-backdrop", "--shadow-lift"];

describe("the motion levels in the stylesheets", () => {
  it("finds the stylesheets", () => {
    expect(Object.keys(components).length).toBeGreaterThan(20);
    expect(rulesOf(base).length).toBeGreaterThan(20);
  });

  it("asks the operating system about reduced motion in one place: the level in effect is on the root element", () => {
    const asking = [...Object.entries(sheets), ...Object.entries(scripts)]
      .filter(([, source]) => source.includes("prefers-reduced-motion"))
      .map(([path]) => path);

    expect(asking).toEqual(["../lib/motion.svelte.ts"]);
  });

  it("the level off zeroes every numbered duration and stops what repeats", () => {
    const root = rulesOf(tokens).find((rule) => rule.selector === ":root")!;
    const off = rulesOf(tokens).find((rule) => rule.selector === `:root${OFF}`)!;
    const durations = [...root.body.matchAll(/(--dur-\d+):/g)].map((match) => match[1]);

    expect(durations).toEqual(["--dur-1", "--dur-2", "--dur-3", "--dur-4", "--dur-5", "--dur-6"]);
    for (const name of durations) expect(off.body).toContain(`${name}: 0ms`);
    expect(root.body).toContain("--loop-state: running");
    expect(off.body).toContain("--loop-state: paused");
  });

  it("every animation that repeats takes its play state from the level", () => {
    for (const [path, css] of Object.entries(sheets)) {
      for (const { selector, body } of rulesOf(css)) {
        for (const declaration of body.split(";").filter((part) => /\binfinite\b/.test(part))) {
          expect(declaration, `${path}: ${selector}`).toContain("var(--loop-state)");
        }
      }
    }
  });

  it("what needs more than zero durations without motion is written under the level off", () => {
    const off = Object.entries(sheets).flatMap(([path, css]) =>
      rulesOf(css)
        .filter((rule) => rule.selector.includes(OFF))
        .map((rule) => `${path}: ${rule.selector}`),
    );

    expect(off).toEqual([
      `tokens.css: :root${OFF}`,
      `base.css: :root${OFF} :is(.fade-in, .pop-in, .scale-in, .rise-in)`,
      `../components/ResponsePanel.svelte: :global(:root${OFF}) .progress::before`,
    ]);
  });

  it("the tokens of the rich level are used under the rich level only, so the normal level is as it always was", () => {
    for (const [path, css] of Object.entries(sheets)) {
      for (const { selector, body } of rulesOf(css)) {
        // Where the tokens are declared and zeroed.
        if (path === "tokens.css") continue;
        const used = RICH_TOKENS.filter((name) => new RegExp(`var\\(${name}[,)]`).test(body));
        if (used.length > 0) expect(selector, `${path} uses ${used.join(", ")}`).toContain(RICH);
      }
    }
  });

  it("every rule of the rich level starts at the root element", () => {
    const rich = Object.entries(sheets).flatMap(([path, css]) =>
      rulesOf(css)
        .filter((rule) => rule.selector.includes(RICH))
        .map((rule) => ({ path, selectors: rule.selector.split(/,(?![^(]*\))/).map((selector) => selector.trim()) })),
    );

    expect(rich.length).toBeGreaterThan(30);
    for (const { path, selectors } of rich) {
      for (const selector of selectors) {
        expect(selector.startsWith(`:root${RICH}`) || selector.startsWith(`:global(:root${RICH})`), `${path}: ${selector}`).toBe(true);
      }
    }
  });
});
