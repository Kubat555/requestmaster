// Display helpers for sizes, durations and status codes. Numbers use a decimal comma: the interface is Russian.

import { strings } from "./strings";

/** `1.5` -> `1,5`: a fixed number of decimals with a decimal comma. */
function decimal(value: number, digits: number): string {
  return value.toFixed(digits).replace(".", ",");
}

/** Size in bytes as text: `512 Б`, `1,5 КБ`, `20,8 МБ`. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} ${strings.units.bytes}`;
  const kilobytes = bytes / 1024;
  // Compared after rounding so that 1023.96 KB is shown as `1,0 МБ`, not `1024,0 КБ`.
  if (Number(kilobytes.toFixed(1)) < 1024) return `${decimal(kilobytes, 1)} ${strings.units.kilobytes}`;
  return `${decimal(kilobytes / 1024, 1)} ${strings.units.megabytes}`;
}

/** Duration in milliseconds as text: `123 мс`, `1,24 с`. */
export function formatMs(ms: number): string {
  const rounded = Math.round(ms);
  if (rounded < 1000) return `${rounded} ${strings.units.milliseconds}`;
  return `${decimal(ms / 1000, 2)} ${strings.units.seconds}`;
}

/** Colour group of an HTTP status code; matches the `--status-*` tokens. */
export type StatusClass = "ok" | "redirect" | "client" | "server";

/** Group of a status code: below 300 — `ok`, 3xx — `redirect`, 4xx — `client`, 500 and above — `server`. */
export function statusClass(status: number): StatusClass {
  if (status < 300) return "ok";
  if (status < 400) return "redirect";
  if (status < 500) return "client";
  return "server";
}
