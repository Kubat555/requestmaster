<script lang="ts">
  interface Props {
    /** Size in pixels of the panel to the left of the splitter. */
    value: number;
    /** Smallest size in pixels. */
    min: number;
    /** Largest size in pixels. */
    max: number;
    /** Pixels per press of an arrow key. */
    step?: number;
    /** Accessible name: what the splitter resizes. */
    label: string;
  }

  let { value = $bindable(), min, max, step = 16, label }: Props = $props();

  /** Pointer position and size at the start of a drag; `null` when nothing is dragged. */
  let drag: { x: number; value: number } | null = null;

  function set(next: number) {
    value = Math.min(max, Math.max(min, Math.round(next)));
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowLeft") set(value - step);
    else if (event.key === "ArrowRight") set(value + step);
    else return;
    event.preventDefault();
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    drag = { x: event.clientX, value };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function onPointerMove(event: PointerEvent) {
    if (drag) set(drag.value + event.clientX - drag.x);
  }

  function onPointerEnd() {
    drag = null;
  }
</script>

<!-- A focusable separator is a widget (WAI-ARIA "window splitter"): it takes focus and keys. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  role="separator"
  tabindex="0"
  aria-orientation="vertical"
  aria-label={label}
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max}
  onkeydown={onKeydown}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerEnd}
  onpointercancel={onPointerEnd}
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

  .splitter:hover,
  .splitter:focus-visible {
    background: var(--accent);
  }
</style>
