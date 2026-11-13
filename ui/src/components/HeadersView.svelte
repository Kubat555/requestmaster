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
        <tr>
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
    color: var(--text-muted);
    font-family: var(--font-ui);
    font-weight: 600;
  }

  .name {
    width: 30%;
  }
</style>
