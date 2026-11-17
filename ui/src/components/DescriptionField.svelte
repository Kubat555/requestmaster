<script lang="ts">
  import { strings } from "../lib/strings";

  interface Props {
    /** The description, edited in place. */
    value: string;
  }

  let { value = $bindable() }: Props = $props();
</script>

<!-- The description of a collection or a folder. The source text as it is kept: Markdown is not rendered. -->
<textarea class="description" aria-label={strings.collection.description} spellcheck="false" bind:value></textarea>

<style>
  .description {
    display: block;
    width: 100%;
    height: 100%;
    padding: calc(var(--space) * 2);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    resize: none;
  }
</style>
