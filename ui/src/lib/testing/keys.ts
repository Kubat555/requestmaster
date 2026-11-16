import { fireEvent } from "@testing-library/svelte";

/**
 * Enter the way a browser handles it: the key goes to the control that has the focus, and a focused button is
 * pressed by it unless a listener cancelled the key. The test environment does not do the second part itself.
 */
export async function pressEnter() {
  const target = document.activeElement ?? document.body;
  const notCancelled = await fireEvent.keyDown(target, { key: "Enter" });
  if (notCancelled && target instanceof HTMLButtonElement) await fireEvent.click(target);
}
