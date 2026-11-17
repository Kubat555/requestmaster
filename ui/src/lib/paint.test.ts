import { afterEach, describe, expect, it, vi } from "vitest";
import { painted } from "./paint";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("painted", () => {
  it("resolves in a task after the next frame, not before it", async () => {
    vi.useFakeTimers();
    let frame: FrameRequestCallback | undefined;
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      frame = callback;
      return 1;
    });
    const done = vi.fn();
    void painted().then(done);

    await vi.advanceTimersByTimeAsync(50);
    expect(done).not.toHaveBeenCalled();

    frame!(0);
    await Promise.resolve();
    expect(done).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(0);
    expect(done).toHaveBeenCalledTimes(1);
  });

  it("does not wait forever for a frame that never comes", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("requestAnimationFrame", () => 1);
    const done = vi.fn();
    void painted().then(done);

    await vi.advanceTimersByTimeAsync(100);

    expect(done).toHaveBeenCalledTimes(1);
  });
});
