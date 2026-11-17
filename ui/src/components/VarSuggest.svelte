<script lang="ts">
  import { strings } from "../lib/strings";
  import type { ScopeVariable } from "../lib/types";
  import { describeVariable } from "../lib/variables";

  interface Props {
    /** Id of the list; the id of an item is {@link optionId}. */
    id: string;
    /** The variables offered, best first. */
    items: ScopeVariable[];
    /** Index of the item the keys of the field act on. */
    active: number;
    /** Left, top and bottom edges of the field, in the coordinates of the window. */
    left: number;
    fieldTop: number;
    fieldBottom: number;
    /** An item was chosen with the mouse. */
    onpick: (variable: ScopeVariable) => void;
  }

  let { id, items, active, left, fieldTop, fieldBottom, onpick }: Props = $props();

  /** Space between the field and the list, px. */
  const GAP = 4;
  /** The greatest height of the list, px; the same as in the styles. */
  const MAX_HEIGHT = 240;

  let list: HTMLDivElement;

  /** Under the field; above it when the whole list does not fit under and there is more room above. */
  const under = $derived.by(() => {
    const roomUnder = window.innerHeight - fieldBottom;
    return roomUnder >= MAX_HEIGHT + GAP || roomUnder >= fieldTop;
  });

  // The item chosen with the arrows is kept in view. The test environment has no layout and no such method.
  $effect(() => {
    list.children[active]?.scrollIntoView?.({ block: "nearest" });
  });
</script>

<script lang="ts" module>
  /** Id of the item `index` of the list `id`: the field names the chosen item with `aria-activedescendant`. */
  export function optionId(id: string, index: number): string {
    return `${id}-${index}`;
  }
</script>

<!-- The variables that fit the name being typed. The focus stays in the field: its keys choose here, and a press
     of the mouse must not take the focus away. -->
<div
  bind:this={list}
  class="var-suggest pop-in"
  role="listbox"
  {id}
  aria-label={strings.variables.suggestions}
  style:left="{left}px"
  style:top={under ? `${fieldBottom + GAP}px` : null}
  style:bottom={under ? null : `${window.innerHeight - fieldTop + GAP}px`}
  style:max-height="{MAX_HEIGHT}px"
>
  {#each items as item, index (item.name)}
    {@const described = describeVariable(item)}
    <div
      class="option"
      class:active={index === active}
      role="option"
      id={optionId(id, index)}
      tabindex="-1"
      aria-selected={index === active}
      onmousedown={(event) => {
        event.preventDefault();
        onpick(item);
      }}
    >
      <span class="name">{item.name}</span>
      <span class={["scope", `scope-${item.scope}`]}>{described.scope}</span>
      <span class="value">{described.value}</span>
    </div>
  {/each}
</div>

<style>
  .var-suggest {
    position: fixed;
    z-index: 90;
    min-width: 280px;
    max-width: 560px;
    padding: var(--space);
    overflow-y: auto;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-popup);
    box-shadow: var(--shadow-popup);
    color: var(--text);
  }

  .option {
    display: flex;
    align-items: baseline;
    gap: calc(var(--space) * 3);
    height: var(--control-height-sm);
    padding: 0 calc(var(--space) * 2);
    border-radius: var(--radius);
    line-height: var(--control-height-sm);
    white-space: nowrap;
    cursor: pointer;
  }

  .option:hover,
  .option.active {
    background: var(--surface);
  }

  .option.active {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }

  .name,
  .value {
    font-family: var(--font-mono);
  }

  .name {
    flex: none;
  }

  .scope {
    flex: none;
  }

  .scope-global {
    color: var(--var-global);
  }

  .scope-collection {
    color: var(--var-collection);
  }

  .scope-environment {
    color: var(--var-environment);
  }

  .scope-dynamic {
    color: var(--var-dynamic);
  }

  .value {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text-muted);
    text-align: right;
    text-overflow: ellipsis;
  }
</style>
