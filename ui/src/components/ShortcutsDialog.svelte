<script lang="ts">
  import Dialog from "./Dialog.svelte";
  import Button from "./ui/Button.svelte";
  import { SHORTCUTS } from "../lib/hotkeys";
  import { strings } from "../lib/strings";

  interface Props {
    /** Called when the dialog must go away: Escape or the button. The owner removes it. */
    onclose: () => void;
  }

  let { onclose }: Props = $props();
</script>

<!-- The list of the key combinations, by the part of the window. The rows are the data of the hotkeys module. -->
<Dialog title={strings.shortcuts.title} {onclose}>
  {#each SHORTCUTS as group (group.title)}
    <section class="group" aria-label={group.title}>
      <h3>{group.title}</h3>
      <dl>
        {#each group.shortcuts as shortcut, row (row)}
          <div class="row">
            <dt>
              {#each shortcut.keys as combination, index (index)}
                {#if index > 0}
                  <span class="between">{shortcut.then ? strings.shortcuts.then : "/"}</span>
                {/if}
                <span class="combination">
                  {#each combination as key, at (at)}
                    {#if at > 0}<span class="plus">+</span>{/if}<kbd>{key}</kbd>
                  {/each}
                </span>
              {/each}
            </dt>
            <dd>{shortcut.text}</dd>
          </div>
        {/each}
      </dl>
    </section>
  {/each}
  <div class="dialog-actions">
    <Button data-initial-focus onclick={onclose}>{strings.shortcuts.close}</Button>
  </div>
</Dialog>

<style>
  .group {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space) * 2);
  }

  h3 {
    margin: 0;
    font-size: var(--font-size);
    font-weight: 600;
    color: var(--text-muted);
  }

  dl {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space) * 1.5);
    margin: 0;
  }

  /* The keys on the left, in a column of one width for the whole list; what they do — on the right. */
  .row {
    display: grid;
    grid-template-columns: 45% minmax(0, 1fr);
    align-items: baseline;
    gap: calc(var(--space) * 3);
  }

  dt {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space) calc(var(--space) * 1.5);
  }

  dd {
    margin: 0;
  }

  .combination {
    display: inline-flex;
    align-items: center;
    gap: calc(var(--space) / 2);
    white-space: nowrap;
  }

  .between,
  .plus {
    color: var(--text-muted);
  }

  kbd {
    padding: 0 calc(var(--space) * 1.5);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 20px;
  }
</style>
