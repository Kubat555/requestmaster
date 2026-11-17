<script lang="ts">
  import type { ScopeVariable } from "../lib/types";
  import { describeVariable } from "../lib/variables";

  interface Props {
    /** Id of the hint: the field refers to it while the hint is shown. */
    id: string;
    /** Name of the variable, without the braces. */
    name: string;
    /** The variable the name resolves to; `null` — there is none. */
    variable: ScopeVariable | null;
    /** Left edge of the token and the line under it, in the coordinates of the window. */
    left: number;
    top: number;
  }

  let { id, name, variable, left, top }: Props = $props();

  const described = $derived(describeVariable(variable));
</script>

<!-- What a `{{name}}` under the pointer stands for: the scope is said in words, the colour only repeats it. -->
<div class="var-tooltip pop-in" role="tooltip" {id} style:left="{left}px" style:top="{top}px">
  <div class="head">
    <span class="name">{name}</span>
    <span class={["scope", variable ? `scope-${variable.scope}` : "scope-unresolved"]}>{described.scope}</span>
  </div>
  {#if described.value !== ""}
    <div class="value">{described.value}</div>
  {/if}
</div>

<style>
  .var-tooltip {
    position: fixed;
    z-index: 90;
    max-width: 420px;
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-popup);
    box-shadow: var(--shadow-popup);
    color: var(--text);
    pointer-events: none;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: calc(var(--space) * 2);
  }

  .name,
  .value {
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .name {
    font-weight: 600;
  }

  .scope {
    flex: none;
    color: var(--text-muted);
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

  .scope-unresolved {
    color: var(--danger);
  }

  .value {
    margin-top: var(--space);
    /* A long value is cut, not the window covered with it. */
    max-height: 120px;
    overflow: hidden;
    white-space: pre-wrap;
  }
</style>
