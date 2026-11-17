// The mark of the chosen tab that slides from tab to tab at the rich motion level: one element in a row of tabs,
// moved by a transform. Used as an attachment of the row (`{@attach slideIndicator}`); at any other level it
// does nothing and the row keeps its static mark. The look of the element is the business of the stylesheets
// (`.slide-indicator`): here it only learns where the chosen tab stands.

import { motion } from "./motion.svelte";

/** The chosen tab of a row. */
const CHOSEN = '[aria-selected="true"]';
/** The element whose place the mark takes, when that is not the chosen tab itself but something around it. */
const ITEM = "[data-slide-item]";

/**
 * Puts the mark into the row and keeps it at the chosen tab: when another tab is chosen, when tabs come, go or
 * change their text, and when the row is resized. The mark is a child of the row, so it scrolls with the tabs.
 * The places are read from the layout (`offsetLeft`, `offsetWidth`) and handed over as CSS variables. Returns
 * what takes the mark away again.
 */
export function slideIndicator(row: HTMLElement): (() => void) | undefined {
  // Read here, so that the attachment runs again when the level changes.
  if (!motion.rich) return undefined;

  const mark = document.createElement("span");
  mark.className = "slide-indicator";
  mark.setAttribute("aria-hidden", "true");
  /** The mark has a place: from the next frame on it slides instead of jumping. */
  let placed = false;

  const place = () => {
    // A row emptied as a whole loses the mark with its tabs.
    if (mark.parentNode !== row) row.append(mark);
    const chosen = row.querySelector<HTMLElement>(CHOSEN);
    const item = chosen?.closest<HTMLElement>(ITEM) ?? chosen;
    mark.hidden = !item;
    // A row that is not laid out — the tab it stands in is hidden — has no places: the mark keeps the one it
    // had, and is back in it, without sliding there, when the row is shown again.
    if (!item || item.offsetWidth === 0) return;
    mark.style.setProperty("--slide-x", `${item.offsetLeft}px`);
    mark.style.setProperty("--slide-width", String(item.offsetWidth));
    if (placed) return;
    placed = true;
    requestAnimationFrame(() => mark.classList.add("sliding"));
  };

  place();
  const changes = new MutationObserver(place);
  changes.observe(row, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ["aria-selected"] });
  // The test environment has no layout and no observer of sizes.
  const sizes = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(place);
  sizes?.observe(row);

  return () => {
    changes.disconnect();
    sizes?.disconnect();
    mark.remove();
  };
}
