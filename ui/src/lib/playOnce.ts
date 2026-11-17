// One-shot animations. One that a class stands for: the class is put on the element and taken away again when
// the animation is over, so that showing the element later (a hidden tab, say) does not play it once more. And
// one of an element that lives only as long as its animation: the element tells when it is over.

import type { Attachment } from "svelte/attachments";

/** The animation of an element is over when it has ended or was cut short: the element was hidden, the motion level changed. */
const OVER = ["animationend", "animationcancel"];

/**
 * Plays the animation of the class on the element. Does nothing while it is still playing. The class goes away
 * when the animation has ended or was cut short — the element was hidden, the motion level changed.
 */
export function playOnce(element: Element, className: string): void {
  if (element.classList.contains(className)) return;
  const done = (event: Event) => {
    // The animations of what is inside the element come up here as well.
    if (event.target !== element) return;
    element.classList.remove(className);
    for (const name of OVER) element.removeEventListener(name, done);
  };
  for (const name of OVER) element.addEventListener(name, done);
  element.classList.add(className);
}

/**
 * For an element that is on the screen only while its animation plays (`{@attach whenAnimationOver(...)}`):
 * calls `over` when the animation of the element itself has ended or was cut short.
 */
export function whenAnimationOver(over: () => void): Attachment {
  return (element) => {
    const done = (event: Event) => {
      if (event.target === element) over();
    };
    for (const name of OVER) element.addEventListener(name, done);
    return () => {
      for (const name of OVER) element.removeEventListener(name, done);
    };
  };
}
