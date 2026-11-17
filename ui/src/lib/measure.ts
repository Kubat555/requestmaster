// The measuring of the window, on only in a program started to be measured (`RQM_MEASURE_DIR`): when the
// window is ready and how long it takes to answer a key press, an input and a click on a tab. The numbers go
// to the core, which appends them to the file of timings.

import type { Api } from "./api";
import type { TimingSample } from "./types";

/** How often the collected measurements are sent. */
export const MEASURE_FLUSH_MS = 2000;

/**
 * The shortest duration the browser reports an event from, and the least it lets ask for: an event that took
 * less does not come at all.
 */
const DURATION_THRESHOLD_MS = 16;

/** The events that are written down wherever they happen. */
const TYPED = new Set(["keydown", "input"]);
/** The only place whose clicks are written down: the title of a tab. */
const TAB = "tab";
/** The place of an event that happened outside every marked one, or has no target. */
const OTHER = "other";

/** The place of the event: `data-measure` of the closest ancestor of its target. */
function placeOf(target: Node | null): string {
  const marked = target instanceof Element ? target.closest("[data-measure]") : null;
  return marked?.getAttribute("data-measure") || OTHER;
}

/** Starts the measuring; the returned function stops it (no more samples are collected or sent). */
export function installMeasure(api: Api): () => void {
  let stopped = false;
  let collected: TimingSample[] = [];
  let timer: ReturnType<typeof setTimeout> | null = null;

  /** A refusal is silent: the measuring must not show itself in the window it measures. */
  const report = (samples: TimingSample[]) => void api.reportTimings(samples).catch(() => {});

  // Two frames later the window as it is now has been drawn. Sent at once, with a call of its own: the
  // script that measures the start waits for this line.
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      if (!stopped) report([{ name: "ready", ms: performance.now() }]);
    }),
  );

  /** Two lines for an event: the time to the next paint, and the wait in the queue plus the handlers alone. */
  const collect = (entry: PerformanceEventTiming) => {
    if (stopped) return;
    const place = placeOf(entry.target);
    if (!TYPED.has(entry.name) && !(entry.name === "click" && place === TAB)) return;
    const name = `${entry.name}:${place}`;
    collected.push({ name, ms: entry.duration }, { name: `${name}:proc`, ms: entry.processingEnd - entry.startTime });
  };

  /** Sends what was collected, if anything, and waits for the next time. `setTimeout` again, not `setInterval`. */
  const schedule = () => {
    timer = setTimeout(() => {
      if (collected.length > 0) {
        report(collected);
        collected = [];
      }
      schedule();
    }, MEASURE_FLUSH_MS);
  };

  let observer: PerformanceObserver | null = null;
  if (typeof PerformanceObserver !== "undefined" && PerformanceObserver.supportedEntryTypes?.includes("event")) {
    observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) collect(entry as PerformanceEventTiming);
    });
    // `durationThreshold` is not in the type of the options yet.
    observer.observe({ type: "event", durationThreshold: DURATION_THRESHOLD_MS } as PerformanceObserverInit);
    schedule();
  }

  return () => {
    stopped = true;
    observer?.disconnect();
    if (timer !== null) clearTimeout(timer);
    timer = null;
    collected = [];
  };
}
