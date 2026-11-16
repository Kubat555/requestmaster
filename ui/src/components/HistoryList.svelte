<script lang="ts">
  import { getContext, onMount } from "svelte";
  import Dialog from "./Dialog.svelte";
  import Button from "./ui/Button.svelte";
  import EmptyState from "./ui/EmptyState.svelte";
  import IconButton from "./ui/IconButton.svelte";
  import type { Api } from "../lib/api";
  import { dayLabel, formatClock, formatMs, methodClass, methodLabel, statusClass } from "../lib/format";
  import { history } from "../lib/history.svelte";
  import { strings } from "../lib/strings";
  import { historyTabId, tabs } from "../lib/tabs.svelte";
  import type { HistoryItem } from "../lib/types";

  const api = getContext<Api>("api");

  const id = $props.id();
  /** The question about deleting the whole history is on the screen. */
  let clearing = $state(false);

  /** The entries by the local day they were sent on; both the days and the entries of a day newest first. */
  const days = $derived.by(() => {
    const now = new Date();
    const groups: { label: string; items: HistoryItem[] }[] = [];
    for (const item of history.items) {
      const label = dayLabel(item.sent_at, now);
      const last = groups.at(-1);
      if (last?.label === label) last.items.push(item);
      else groups.push({ label, items: [item] });
    }
    return groups;
  });

  // The section was opened: the sends made while it was closed are read.
  onMount(() => void history.reload(api));

  /** The user confirmed the deletion: the tabs of the entries have nothing to show any more. */
  async function confirmClear() {
    clearing = false;
    if (await history.clear(api)) tabs.closeHistory();
  }
</script>

<div class="toolbar">
  <IconButton icon="trash" label={strings.history.clear} disabled={history.items.length === 0} onclick={() => (clearing = true)} />
</div>
<div class="content">
  {#if history.items.length === 0}
    <EmptyState icon="history" title={strings.sidebar.historyEmpty} />
  {:else}
    {#each days as day (day.items[0].id)}
      {@const headingId = `${id}-day-${day.items[0].id}`}
      <h3 id={headingId} class="day">{day.label}</h3>
      <ul aria-labelledby={headingId}>
        {#each day.items as item (item.id)}
          <li>
            <!-- The address is in the tooltip: the row has room for the name only. -->
            <button
              type="button"
              class="row"
              class:selected={tabs.active?.id === historyTabId(item.id)}
              data-history={item.id}
              title={item.url}
              onclick={() => void tabs.openHistory(api, item.id)}
            >
              <span class="clock">{formatClock(item.sent_at)}</span>
              <span class={["method", methodClass(item.method)]}>{methodLabel(item.method)}</span>
              <span class="name">{item.name}</span>
              {#if item.error_kind !== null}
                <span class="failure">{strings.history.errors[item.error_kind]}</span>
              {:else}
                <!-- The colour repeats what the code says. -->
                {#if item.status !== null}
                  <span class={["status", statusClass(item.status)]}>{item.status}</span>
                {/if}
                {#if item.total_ms !== null}
                  <span class="time">{formatMs(item.total_ms)}</span>
                {/if}
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/each}
    {#if history.hasMore}
      <div class="more">
        <Button variant="ghost" size="sm" onclick={() => void history.loadMore(api)}>{strings.history.more}</Button>
      </div>
    {/if}
  {/if}
</div>

{#if clearing}
  <Dialog title={strings.history.clearTitle} onclose={() => (clearing = false)}>
    <p class="question">{strings.history.clearText}</p>
    <div class="dialog-actions">
      <Button class="danger" onclick={confirmClear}>{strings.history.clearConfirm}</Button>
      <Button data-initial-focus onclick={() => (clearing = false)}>{strings.history.cancel}</Button>
    </div>
  </Dialog>
{/if}

<style>
  .toolbar {
    flex: none;
    display: flex;
    justify-content: flex-end;
    padding: calc(var(--space) * 2);
    border-bottom: 1px solid var(--border);
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .day {
    margin: 0;
    padding: calc(var(--space) * 2) calc(var(--space) * 3) var(--space);
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 2);
    width: 100%;
    height: var(--row-height);
    padding: 0 calc(var(--space) * 3);
    background: transparent;
    border: none;
    text-align: left;
    white-space: nowrap;
    cursor: default;
  }

  /* Hover and selection switch at once, without a transition. */
  .row:hover {
    background: color-mix(in srgb, var(--surface-2) 60%, transparent);
  }

  .row.selected {
    background: var(--surface-2);
  }

  .clock,
  .time,
  .failure {
    flex: none;
    color: var(--text-muted);
  }

  .clock,
  .method,
  .status {
    flex: none;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .method,
  .status {
    font-weight: 600;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ok {
    color: var(--status-ok);
  }

  .redirect {
    color: var(--status-redirect);
  }

  .client {
    color: var(--status-client);
  }

  .server {
    color: var(--status-server);
  }

  .more {
    display: flex;
    justify-content: center;
    padding: calc(var(--space) * 2);
  }

  .question {
    margin: 0;
  }

  /* The button that destroys. The class goes to the `<button>` of `Button`, hence `:global`. */
  .dialog-actions :global(.danger) {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
