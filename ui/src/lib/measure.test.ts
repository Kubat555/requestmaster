import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { installMeasure, MEASURE_FLUSH_MS } from "./measure";
import { createFakeApi } from "./testing/fakeApi";

/** What a test tells about one event of the page. */
interface EntrySeed {
  name: string;
  startTime: number;
  processingEnd: number;
  duration: number;
  target?: Element | null;
}

/** The observers the code under test has created, in order; a test gives them entries itself. */
let observers: FakeObserver[] = [];

/** Stands for `PerformanceObserver`: remembers the handler and what it was asked to observe. */
class FakeObserver {
  static supportedEntryTypes: string[] = ["event"];
  observed: unknown[] = [];
  disconnected = false;
  readonly #handler: (list: { getEntries(): unknown[] }) => void;

  constructor(handler: (list: { getEntries(): unknown[] }) => void) {
    this.#handler = handler;
    observers.push(this);
  }

  observe(options: unknown): void {
    this.observed.push(options);
  }

  disconnect(): void {
    this.disconnected = true;
  }

  /** The page had these events. */
  emit(...entries: EntrySeed[]): void {
    this.#handler({ getEntries: () => entries.map((entry) => ({ target: null, ...entry })) });
  }
}

/** A real element inside `[data-measure="<label>"]`; without a label — inside a plain element. */
function targetIn(label: string | null): Element {
  const place = document.createElement("div");
  if (label !== null) place.setAttribute("data-measure", label);
  const target = place.appendChild(document.createElement("span")).appendChild(document.createElement("input"));
  document.body.append(place);
  return target;
}

const READY = [{ name: "ready", ms: expect.any(Number) }];

describe("installMeasure", () => {
  let stop: (() => void) | undefined;

  beforeEach(() => {
    observers = [];
    FakeObserver.supportedEntryTypes = ["event"];
    vi.stubGlobal("PerformanceObserver", FakeObserver);
    // Two frames pass at once.
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      callback(0);
      return 0;
    });
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  });

  afterEach(() => {
    stop?.();
    stop = undefined;
    vi.useRealTimers();
    vi.unstubAllGlobals();
    document.body.replaceChildren();
  });

  it("reports `ready` once, with a call of its own", async () => {
    const api = createFakeApi();
    stop = installMeasure(api);

    expect(api.timingCalls).toEqual([READY]);
    expect(api.timingCalls[0][0].ms).toBeGreaterThan(0);
    await vi.advanceTimersByTimeAsync(3 * MEASURE_FLUSH_MS);
    expect(api.timingCalls).toEqual([READY]);
  });

  it("observes the events that took 16 ms or more", () => {
    stop = installMeasure(createFakeApi());

    expect(MEASURE_FLUSH_MS).toBe(2000);
    expect(observers).toHaveLength(1);
    expect(observers[0].observed).toEqual([{ type: "event", durationThreshold: 16 }]);
  });

  it("sends a click on the title of a tab after 2000 ms, with one call", async () => {
    const api = createFakeApi();
    stop = installMeasure(api);
    observers[0].emit({ name: "click", startTime: 200, processingEnd: 230, duration: 64, target: targetIn("tab") });

    await vi.advanceTimersByTimeAsync(1999);
    expect(api.timingCalls).toEqual([READY]);
    await vi.advanceTimersByTimeAsync(1);
    expect(api.timingCalls).toEqual([
      READY,
      [
        { name: "click:tab", ms: 64 },
        { name: "click:tab:proc", ms: 30 },
      ],
    ]);
  });

  it("names a key press by the place it was made in", async () => {
    const api = createFakeApi();
    stop = installMeasure(api);
    observers[0].emit({ name: "keydown", startTime: 100, processingEnd: 107, duration: 24, target: targetIn("url") });
    observers[0].emit(
      { name: "keydown", startTime: 300, processingEnd: 302, duration: 16, target: targetIn(null) },
      { name: "input", startTime: 400, processingEnd: 409, duration: 32, target: targetIn("body") },
      { name: "keydown", startTime: 500, processingEnd: 501, duration: 16 },
    );

    await vi.advanceTimersByTimeAsync(MEASURE_FLUSH_MS);
    expect(api.timingCalls).toEqual([
      READY,
      [
        { name: "keydown:url", ms: 24 },
        { name: "keydown:url:proc", ms: 7 },
        { name: "keydown:other", ms: 16 },
        { name: "keydown:other:proc", ms: 2 },
        { name: "input:body", ms: 32 },
        { name: "input:body:proc", ms: 9 },
        { name: "keydown:other", ms: 16 },
        { name: "keydown:other:proc", ms: 1 },
      ],
    ]);
  });

  it("does not send a key release, a click outside a tab title or a pointer event", async () => {
    const api = createFakeApi();
    stop = installMeasure(api);
    observers[0].emit(
      { name: "keyup", startTime: 100, processingEnd: 107, duration: 24, target: targetIn("url") },
      { name: "click", startTime: 200, processingEnd: 230, duration: 64, target: targetIn("tree") },
      { name: "click", startTime: 200, processingEnd: 230, duration: 64, target: targetIn(null) },
      { name: "pointerdown", startTime: 300, processingEnd: 330, duration: 64, target: targetIn("tab") },
    );

    await vi.advanceTimersByTimeAsync(MEASURE_FLUSH_MS);
    expect(api.timingCalls).toEqual([READY]);
  });

  it("sends the next batch 2000 ms after the previous one", async () => {
    const api = createFakeApi();
    stop = installMeasure(api);
    observers[0].emit({ name: "keydown", startTime: 100, processingEnd: 107, duration: 24, target: targetIn("url") });
    await vi.advanceTimersByTimeAsync(MEASURE_FLUSH_MS);
    observers[0].emit({ name: "input", startTime: 100, processingEnd: 105, duration: 16, target: targetIn("tree-filter") });
    await vi.advanceTimersByTimeAsync(MEASURE_FLUSH_MS);

    expect(api.timingCalls).toHaveLength(3);
    expect(api.timingCalls[2]).toEqual([
      { name: "input:tree-filter", ms: 16 },
      { name: "input:tree-filter:proc", ms: 5 },
    ]);
  });

  it("reports only `ready` where events are not measured", async () => {
    FakeObserver.supportedEntryTypes = [];
    const api = createFakeApi();
    stop = installMeasure(api);

    expect(observers).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(MEASURE_FLUSH_MS);
    expect(api.timingCalls).toEqual([READY]);
  });

  it("sends nothing after it was stopped", async () => {
    const api = createFakeApi();
    stop = installMeasure(api);
    observers[0].emit({ name: "keydown", startTime: 100, processingEnd: 107, duration: 24, target: targetIn("url") });
    stop();
    observers[0].emit({ name: "keydown", startTime: 200, processingEnd: 207, duration: 24, target: targetIn("url") });

    expect(observers[0].disconnected).toBe(true);
    await vi.advanceTimersByTimeAsync(2 * MEASURE_FLUSH_MS);
    expect(api.timingCalls).toEqual([READY]);
  });

  it("keeps measuring when a report is refused", async () => {
    const api = createFakeApi();
    api.reportTimings = vi.fn(() => Promise.reject("cannot write the timings"));
    stop = installMeasure(api);
    observers[0].emit({ name: "keydown", startTime: 100, processingEnd: 107, duration: 24, target: targetIn("url") });
    await vi.advanceTimersByTimeAsync(MEASURE_FLUSH_MS);
    observers[0].emit({ name: "keydown", startTime: 200, processingEnd: 207, duration: 24, target: targetIn("url") });
    await vi.advanceTimersByTimeAsync(MEASURE_FLUSH_MS);

    expect(api.reportTimings).toHaveBeenCalledTimes(3);
  });
});
