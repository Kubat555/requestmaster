import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { CLIP_PULSE, installFieldEffects, TICK_DRAW } from "./fieldEffects";
import { motion } from "./motion.svelte";
import { playOnce } from "./playOnce";

/** A clipboard event the way the webview sends it: from the field, up to the document. */
function clipboard(target: Element, type: "copy" | "cut" | "paste"): Event {
  const event = new Event(type, { bubbles: true, cancelable: true });
  target.dispatchEvent(event);
  return event;
}

function input(value: string, selection?: [number, number]): HTMLInputElement {
  const field = document.createElement("input");
  field.type = "text";
  field.value = value;
  document.body.append(field);
  if (selection) field.setSelectionRange(...selection);
  else field.setSelectionRange(value.length, value.length);
  return field;
}

let stop: () => void;

beforeEach(() => {
  motion.choose("rich");
  stop = installFieldEffects();
});

afterEach(() => {
  stop();
  motion.choose("normal");
  document.body.replaceChildren();
});

describe("playOnce", () => {
  it("puts the class on and takes it away when the animation has ended", () => {
    const element = document.createElement("div");
    playOnce(element, "shake");
    expect(element.classList.contains("shake")).toBe(true);

    element.dispatchEvent(new Event("animationend"));
    expect(element.classList.contains("shake")).toBe(false);

    // And again, as many times as asked.
    playOnce(element, "shake");
    expect(element.classList.contains("shake")).toBe(true);
    element.dispatchEvent(new Event("animationcancel"));
    expect(element.classList.contains("shake")).toBe(false);
  });

  it("is not ended by an animation of something inside the element", () => {
    const element = document.createElement("div");
    const child = element.appendChild(document.createElement("span"));
    playOnce(element, "shake");

    child.dispatchEvent(new Event("animationend", { bubbles: true }));
    expect(element.classList.contains("shake")).toBe(true);
  });
});

describe("clipboard pulse", () => {
  it("pulses the field on a paste and on a copy or a cut of a selection", () => {
    for (const type of ["paste", "copy", "cut"] as const) {
      const field = input("https://example.test", [0, 5]);

      const event = clipboard(field, type);

      expect(field.classList.contains(CLIP_PULSE), type).toBe(true);
      // The event is only seen: the webview does the copying and the pasting.
      expect(event.defaultPrevented).toBe(false);
      field.dispatchEvent(new Event("animationend"));
      expect(field.classList.contains(CLIP_PULSE), type).toBe(false);
    }
  });

  it("pulses the frame of the field when it has one", () => {
    const frame = document.createElement("span");
    frame.setAttribute("data-pulse-frame", "");
    const field = input("text");
    document.body.append(frame);
    frame.append(field);

    clipboard(field, "paste");

    expect(frame.classList.contains(CLIP_PULSE)).toBe(true);
    expect(field.classList.contains(CLIP_PULSE)).toBe(false);
  });

  it("does not pulse on a copy or a cut without a selection", () => {
    const field = input("text");

    clipboard(field, "copy");
    clipboard(field, "cut");

    expect(field.classList.contains(CLIP_PULSE)).toBe(false);
  });

  it("leaves alone what is not a one-line field or a text area", () => {
    const editor = document.body.appendChild(document.createElement("div"));
    editor.className = "cm-content";
    editor.contentEditable = "true";

    clipboard(editor, "paste");
    clipboard(document.body, "copy");

    expect(editor.classList.contains(CLIP_PULSE)).toBe(false);
    expect(document.body.classList.contains(CLIP_PULSE)).toBe(false);
  });

  it.each(["off", "normal"] as const)("does nothing at the motion level %j", (level) => {
    motion.choose(level);
    const field = input("text", [0, 4]);

    clipboard(field, "paste");
    clipboard(field, "copy");

    expect(field.classList.contains(CLIP_PULSE)).toBe(false);
  });

  it("stops when it is told to", () => {
    stop();
    const field = input("text");

    clipboard(field, "paste");

    expect(field.classList.contains(CLIP_PULSE)).toBe(false);
  });
});

describe("the mark of a check box", () => {
  function box(checked: boolean): HTMLInputElement {
    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = checked;
    document.body.append(input);
    return input;
  }

  /** The user toggles the box. */
  function toggle(input: HTMLInputElement) {
    input.click();
  }

  it("is drawn when the user checks the box, and only then", () => {
    const input = box(false);

    toggle(input);
    expect(input.checked).toBe(true);
    expect(input.classList.contains(TICK_DRAW)).toBe(true);

    input.dispatchEvent(new Event("animationend"));
    expect(input.classList.contains(TICK_DRAW)).toBe(false);

    // Taking the mark away draws nothing.
    toggle(input);
    expect(input.checked).toBe(false);
    expect(input.classList.contains(TICK_DRAW)).toBe(false);
  });

  it("is not drawn for a box that was checked by the program", () => {
    const input = box(true);

    expect(input.classList.contains(TICK_DRAW)).toBe(false);
  });

  it("leaves other controls alone", () => {
    const radio = document.body.appendChild(document.createElement("input"));
    radio.type = "radio";

    radio.click();

    expect(radio.classList.contains(TICK_DRAW)).toBe(false);
  });

  it.each(["off", "normal"] as const)("is not drawn at the motion level %j", (level) => {
    motion.choose(level);
    const input = box(false);

    toggle(input);

    expect(input.classList.contains(TICK_DRAW)).toBe(false);
  });
});
