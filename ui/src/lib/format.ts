// Display helpers for sizes, durations, moments and status codes. Numbers use a decimal comma: the interface is
// Russian.

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

/** `7` -> `07`. */
function twoDigits(value: number): string {
  return String(value).padStart(2, "0");
}

/** The local time of a moment given as ISO text (`2026-11-13T10:42:07.123Z`): `ЧЧ:ММ:СС`. */
export function formatClock(iso: string): string {
  const date = new Date(iso);
  return `${twoDigits(date.getHours())}:${twoDigits(date.getMinutes())}:${twoDigits(date.getSeconds())}`;
}

/** The local date of a moment given as ISO text: `ДД.ММ.ГГГГ`. */
export function formatDate(iso: string): string {
  const date = new Date(iso);
  return `${twoDigits(date.getDate())}.${twoDigits(date.getMonth() + 1)}.${date.getFullYear()}`;
}

/** The local day of a moment given as ISO text, as seen at `now`: "today", "yesterday", or the date. */
export function dayLabel(iso: string, now: Date): string {
  const date = new Date(iso);
  // Compared by the calendar, not by 24 hours: one second after midnight is already another day.
  const sameDay = (other: Date) =>
    date.getFullYear() === other.getFullYear() && date.getMonth() === other.getMonth() && date.getDate() === other.getDate();
  if (sameDay(now)) return strings.history.today;
  if (sameDay(new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1))) return strings.history.yesterday;
  return formatDate(iso);
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

/** Methods that have a colour of their own; any other one is drawn with `--method-other`. */
const COLOURED_METHODS = ["get", "post", "put", "patch", "delete"];
/** A longer method is cut so that the names next to it stay aligned. */
const METHOD_MAX = 6;

/** CSS class of an HTTP method, the same in the tree and on a tab; the rules are in `styles/base.css`. */
export function methodClass(method: string): string {
  const lower = method.toLowerCase();
  return COLOURED_METHODS.includes(lower) ? `method-${lower}` : "method-other";
}

/** The method as it is shown: its first {@link METHOD_MAX} characters. */
export function methodLabel(method: string): string {
  return method.slice(0, METHOD_MAX);
}
