<script lang="ts">
  import { icons, type IconName } from "../../lib/icons";

  interface Props {
    name: IconName;
  }

  let { name }: Props = $props();
</script>

<!-- An outline icon of 16 px in the colour of the text around it. It is decoration, hidden from readers: the
     name comes from the control it stands in. -->
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
  aria-hidden="true"
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
