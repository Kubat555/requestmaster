<script lang="ts">
  interface Props {
    /** `sm` — the size of an icon, for a button; `lg` — for the middle of a panel. */
    size?: "sm" | "lg";
  }

  let { size = "sm" }: Props = $props();
</script>

<!-- A turning ring in the colour of the text around it: something is running. It is decoration, hidden from
     readers: what runs is said by the element it stands in. Without motion the ring stands still (`--loop-state`):
     what stands next to it already says that something runs. -->
<span class={["spinner", size]} aria-hidden="true"></span>

<style>
  .spinner {
    flex: none;
    display: block;
    width: 16px;
    height: 16px;
    border: 2px solid color-mix(in srgb, currentColor 25%, transparent);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin var(--dur-spin) linear infinite var(--loop-state);
  }

  .spinner.lg {
    width: 28px;
    height: 28px;
    border-width: 3px;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
