<script lang="ts">
  interface Props {
    /** Size of the panel before the splitter (after it with `reverse`): pixels, or the unit `scale` gives. */
    value: number;
    /** Smallest size. */
    min: number;
    /** Largest size. */
    max: number;
    /** Change of the size per press of an arrow key. */
    step?: number;
    /** Accessible name: what the splitter resizes. */
    label: string;
    /** Direction of the line itself: `vertical` stands between a left and a right panel, `horizontal` between an upper and a lower one. */
    orientation?: "vertical" | "horizontal";
    /** The resized panel stands after the splitter: moving the splitter forward makes it smaller. */
    reverse?: boolean;
    /** Units of the size in one pixel of the pointer's way, asked at every move; without it the size is in pixels. */
    scale?: () => number;
    /** Digits after the point the size is rounded to. */
    digits?: number;
    /** The size a double click brings back; without it a double click does nothing. */
    reset?: number;
  }

  let { value = $bindable(), min, max, step = 16, label, orientation = "vertical", reverse = false, scale, digits = 0, reset }: Props = $props();

  /** Pointer position and size at the start of a drag; `null` when nothing is dragged. */
  let drag: { at: number; value: number } | null = null;

  const horizontal = $derived(orientation === "horizontal");
  /** +1 when moving the splitter forward (right or down) makes the panel bigger. */
  const direction = $derived(reverse ? -1 : 1);

  function set(next: number) {
    if (Number.isNaN(next)) return;
    value = Math.min(max, Math.max(min, Number(next.toFixed(digits))));
  }

  /** Where the pointer is along the way of the splitter. */
  function position(event: PointerEvent): number {
    return horizontal ? event.clientY : event.clientX;
  }

  /** The arrows move the splitter the way they point. */
  function onKeydown(event: KeyboardEvent) {
    const [back, forward] = horizontal ? ["ArrowUp", "ArrowDown"] : ["ArrowLeft", "ArrowRight"];
    if (event.key === back) set(value - direction * step);
    else if (event.key === forward) set(value + direction * step);
    else return;
    event.preventDefault();
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    drag = { at: position(event), value };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function onPointerMove(event: PointerEvent) {
    if (drag) set(drag.value + direction * (position(event) - drag.at) * (scale?.() ?? 1));
  }

  function onPointerEnd() {
    drag = null;
  }

  function onDoubleClick() {
    if (reset !== undefined) set(reset);
  }
</script>

<!-- A focusable separator is a widget (WAI-ARIA "window splitter"): it takes focus and keys. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  class:horizontal
  role="separator"
  tabindex="0"
  aria-orientation={orientation}
  aria-label={label}
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max}
  onkeydown={onKeydown}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerEnd}
  onpointercancel={onPointerEnd}
  ondblclick={onDoubleClick}
></div>

<style>
  .splitter {
    flex: none;
    width: 5px;
    margin: 0 -2px;
    z-index: 1;
    cursor: col-resize;
    touch-action: none;
    background: transparent;
    transition: background var(--dur-2) var(--ease);
  }

  .splitter.horizontal {
    width: auto;
    height: 5px;
    margin: -2px 0;
    cursor: row-resize;
  }

  .splitter:hover,
  .splitter:focus-visible {
    background: var(--accent);
  }
</style>
