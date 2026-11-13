import { describe, expect, it } from "vitest";
import css from "./tokens.css?raw";

/** Value of a colour token as written in tokens.css, e.g. `#14171c`. */
function token(name: string): string {
  const match = new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})\\b`).exec(css);
  if (!match) throw new Error(`token ${name} is not a #rrggbb colour in tokens.css`);
  return match[1];
}

/** Names of all colour tokens that start with the prefix, e.g. `--method-`. */
function tokensWithPrefix(prefix: string): string[] {
  const names = [...css.matchAll(new RegExp(`(${prefix}[a-z0-9-]+):\\s*#`, "g"))].map((m) => m[1]);
  return [...new Set(names)];
}

/** Relative luminance of a `#rrggbb` colour (WCAG 2.x). */
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Contrast ratio of two colours (WCAG 2.x), 1..21. */
function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

describe("colour tokens", () => {
  const pairs: [string, string][] = [
    ["--text", "--bg"],
    ["--text", "--surface"],
    ["--text-muted", "--surface"],
    ...tokensWithPrefix("--method-").map((name): [string, string] => [name, "--surface"]),
    ...tokensWithPrefix("--status-").map((name): [string, string] => [name, "--surface"]),
    ["--accent-contrast", "--accent"],
    // Text of the code editor lies on the background of the window.
    ...["--code-key", "--code-string", "--code-number", "--code-atom"].map((name): [string, string] => [name, "--bg"]),
  ];

  it("finds every method and status colour", () => {
    expect(tokensWithPrefix("--method-")).toHaveLength(6);
    expect(tokensWithPrefix("--status-")).toHaveLength(4);
  });

  it.each(pairs)("%s on %s has a contrast of at least 4.5", (foreground, background) => {
    expect(contrast(token(foreground), token(background))).toBeGreaterThanOrEqual(4.5);
  });

  // Line numbers are an aid, not content: the threshold of non-text contrast is enough.
  it("--code-line on --bg has a contrast of at least 3", () => {
    expect(contrast(token("--code-line"), token("--bg"))).toBeGreaterThanOrEqual(3);
  });
});
