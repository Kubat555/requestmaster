// What the plain controls of the window answer with at the rich motion level, wherever they stand: the frame of
// a one-line field pulses once when something is copied from it, cut or pasted, and the mark of a check box is
// drawn when the box is checked. The events are only seen here — nothing is read from the clipboard and nothing
// is held back. The code editor has a pulse of its own (codemirror/typingGlow.ts). The classes are drawn by
// base.css.

import { motion } from "./motion.svelte";
import { playOnce } from "./playOnce";

/** The class of the pulse of a field. */
export const CLIP_PULSE = "clip-pulse";
/** The class of a check box whose mark is being drawn. */
export const TICK_DRAW = "tick-draw";

/** What pulses: the frame around the field when it has one (`data-pulse-frame`), otherwise the field itself. */
function frameOf(field: Element): Element {
  return field.closest("[data-pulse-frame]") ?? field;
}

function onClipboard(event: Event): void {
  if (!motion.rich) return;
  const field = event.target;
  if (!(field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement)) return;
  // Without a selection a copy and a cut take nothing.
  if (event.type !== "paste" && field.selectionStart === field.selectionEnd) return;
  playOnce(frameOf(field), CLIP_PULSE);
}

function onChange(event: Event): void {
  if (!motion.rich) return;
  const box = event.target;
  if (box instanceof HTMLInputElement && box.type === "checkbox" && box.checked) playOnce(box, TICK_DRAW);
}

const CLIPBOARD_EVENTS = ["copy", "cut", "paste"];

/** Starts watching the controls of the window; the returned function stops it. */
export function installFieldEffects(): () => void {
  for (const name of CLIPBOARD_EVENTS) document.addEventListener(name, onClipboard);
  document.addEventListener("change", onChange);
  return () => {
    for (const name of CLIPBOARD_EVENTS) document.removeEventListener(name, onClipboard);
    document.removeEventListener("change", onChange);
  };
}
