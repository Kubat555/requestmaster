// Waiting for the window to be drawn: a state shown right before long work must reach the screen first.

/** How long the frame is waited for, in milliseconds. */
const FRAME_TIMEOUT_MS = 100;

/**
 * Resolves when the window has drawn what was changed before the call: after the next frame, in a task of its
 * own, so that the frame is on the screen by then. A window that draws no frames (it is hidden) does not keep
 * the caller waiting: the wait ends by itself a little later.
 */
export function painted(): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, FRAME_TIMEOUT_MS);
    requestAnimationFrame(() => {
      clearTimeout(timer);
      setTimeout(resolve, 0);
    });
  });
}
