<script lang="ts">
  import Dialog from "./Dialog.svelte";
  import { strings } from "../lib/strings";
  import type { ImportResult, ReportItem } from "../lib/types";

  interface Props {
    /** Outcome of every imported file, in the order of the files. */
    results: ImportResult[];
    onclose: () => void;
  }

  let { results, onclose }: Props = $props();

  /** Order of the groups of records. */
  const KINDS: ReportItem["kind"][] = ["unsupported_auth", "setting_not_applied", "has_scripts", "file_reference"];

  /** Records of all files grouped by kind; kinds without records are left out. */
  const groups = $derived(
    KINDS.map((kind) => ({
      kind,
      items: results.flatMap((result) => result.report).filter((item) => item.kind === kind),
    })).filter((group) => group.items.length > 0),
  );

  /** What the file turned into, e.g. the kind and the name of a collection. */
  function outcome(result: ImportResult): string {
    if (result.kind === "error") return result.error ?? "";
    const kind = strings.import[result.kind];
    return result.name === null ? kind : `${kind}: ${result.name}`;
  }
</script>

<Dialog title={strings.import.reportTitle} {onclose}>
  <ul class="files">
    {#each results as result, index (index)}
      <li>
        <span class="file">{result.file_name}</span>
        <span class="outcome" class:error={result.kind === "error"}>{outcome(result)}</span>
      </li>
    {/each}
  </ul>
  {#each groups as group (group.kind)}
    <section>
      <h3>{strings.import.reportKinds[group.kind]}</h3>
      <ul class="items">
        {#each group.items as item, index (index)}
          <li>
            <span class="path">{item.node_path}</span>
            {#if item.detail !== ""}
              <span class="detail">{item.detail}</span>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/each}
  <div class="dialog-actions">
    <button type="button" class="primary" onclick={onclose}>{strings.import.close}</button>
  </div>
</Dialog>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    flex-wrap: wrap;
    gap: 0 calc(var(--space) * 3);
    padding: var(--space) 0;
    overflow-wrap: anywhere;
  }

  .files li {
    border-bottom: 1px solid var(--border);
  }

  .file {
    font-weight: 600;
  }

  .outcome.error {
    color: var(--danger);
  }

  h3 {
    margin: 0 0 var(--space);
    font-size: inherit;
    font-weight: 600;
    color: var(--text-muted);
  }

  .detail {
    font-family: var(--font-mono);
    color: var(--text-muted);
  }
</style>
