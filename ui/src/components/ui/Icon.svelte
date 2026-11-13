<script lang="ts">
  import { icons, type IconName } from "../../lib/icons";

  interface Props {
    name: IconName;
    /** What the icon says when it stands alone; without it the icon is decoration and is hidden from readers. */
    label?: string;
  }

  let { name, label }: Props = $props();
</script>

<!-- An outline icon of 16 px in the colour of the text around it. -->
<svg
  class="icon"
  width="16"
  height="16"
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.6"
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
>
  {#each icons[name] as d, index (index)}
    <path {d} />
  {/each}
</svg>

<style>
  .icon {
    flex: none;
    display: block;
  }
</style>
