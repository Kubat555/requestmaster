<script lang="ts">
  import { getContext } from "svelte";
  import Button from "./ui/Button.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import { toasts } from "../lib/toasts.svelte";
  import type { FileRow } from "../lib/types";

  interface Props {
    /** The file that is sent as the body; `null` — not chosen. Replaced as a whole. */
    file: FileRow | null;
  }

  let { file = $bindable() }: Props = $props();

  const api = getContext<Api>("api");

  /**
   * Asks for a file with the system window; a window that was closed changes nothing. Only a path that came
   * from that window is confirmed.
   */
  async function choose() {
    let paths: string[];
    try {
      paths = await api.pickFiles(false);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    if (paths.length > 0) file = { path: paths[0], confirmed: true };
  }
</script>

<!-- The file of a binary body. It is chosen only through the system window; there is no field to type a path into. -->
<div class="binary">
  {#if file}
    <span class="path">{file.path}</span>
    {#if !file.confirmed}
      <span class="unconfirmed">{strings.body.fileUnconfirmed}</span>
    {/if}
  {:else}
    <span class="none">{strings.body.fileNotChosen}</span>
  {/if}
  <Button onclick={choose}>{strings.body.chooseFile}</Button>
  {#if file}
    <Button onclick={() => (file = null)}>{strings.body.removeChosen}</Button>
  {/if}
</div>

<style>
  .binary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: calc(var(--space) * 2);
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
  }

  .path {
    min-width: 0;
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .none {
    color: var(--text-muted);
  }

  .unconfirmed {
    color: var(--danger);
  }
</style>
