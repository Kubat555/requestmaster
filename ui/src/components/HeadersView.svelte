<script lang="ts">
  import { strings } from "../lib/strings";

  interface Props {
    /** Headers of the response as name and value, in the order they were received. */
    headers: [string, string][];
  }

  let { headers }: Props = $props();
</script>

{#if headers.length === 0}
  <p class="note">{strings.response.noHeaders}</p>
{:else}
  <table aria-label={strings.response.headers}>
    <thead>
      <tr>
        <th scope="col">{strings.response.name}</th>
        <th scope="col">{strings.response.value}</th>
      </tr>
    </thead>
    <tbody>
      {#each headers as [name, value], index (index)}
        <tr class="hover-row">
          <td class="name">{name}</td>
          <td>{value}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .note {
    margin: 0;
    color: var(--text-muted);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  th,
  td {
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    border: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
  }

  th {
    background: var(--surface-2);
    color: var(--text);
    font-family: var(--font-ui);
    font-weight: 600;
  }

  .name {
    width: 30%;
  }

  /* The rich motion level: the rows come in one after another. Only the first twelve wait their turn; the rest
     come together right after them, so that a long list does not take long. */
  :global(:root[data-motion="rich"]) tbody tr {
    animation: row-in var(--dur-3) var(--ease) backwards;
    animation-delay: calc(var(--stagger) * 12);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(1) {
    animation-delay: calc(var(--stagger) * 0);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(2) {
    animation-delay: calc(var(--stagger) * 1);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(3) {
    animation-delay: calc(var(--stagger) * 2);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(4) {
    animation-delay: calc(var(--stagger) * 3);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(5) {
    animation-delay: calc(var(--stagger) * 4);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(6) {
    animation-delay: calc(var(--stagger) * 5);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(7) {
    animation-delay: calc(var(--stagger) * 6);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(8) {
    animation-delay: calc(var(--stagger) * 7);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(9) {
    animation-delay: calc(var(--stagger) * 8);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(10) {
    animation-delay: calc(var(--stagger) * 9);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(11) {
    animation-delay: calc(var(--stagger) * 10);
  }

  :global(:root[data-motion="rich"]) tbody tr:nth-child(12) {
    animation-delay: calc(var(--stagger) * 11);
  }

  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
