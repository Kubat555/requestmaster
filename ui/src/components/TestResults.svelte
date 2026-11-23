<script lang="ts">
  import EmptyState from "./ui/EmptyState.svelte";
  import { strings } from "../lib/strings";
  import type { ScriptError, ScriptSource, TestResult } from "../lib/types";

  interface Props {
    /** What the `pm.test` calls of a send recorded, in the order they ran. */
    tests: TestResult[];
    /** The errors that stopped scripts of the send. */
    errors: ScriptError[];
  }

  let { tests, errors }: Props = $props();

  type Outcome = "passed" | "failed" | "skipped";

  /** A skipped test is neither passed nor failed, whatever it says about passing. */
  function outcome(test: TestResult): Outcome {
    if (test.skipped) return "skipped";
    return test.passed ? "passed" : "failed";
  }

  function where(source: ScriptSource): string {
    return strings.tests.where(strings.scripts.kinds[source.phase], source.node_name);
  }

  const totals = $derived.by(() => {
    const totals: Record<Outcome, number> = { passed: 0, failed: 0, skipped: 0 };
    for (const test of tests) totals[outcome(test)] += 1;
    return totals;
  });
</script>

<!-- Everything a script wrote — names, messages — is text and is put into the page as text only. -->
{#if tests.length === 0 && errors.length === 0}
  <EmptyState icon="check" title={strings.tests.empty} text={strings.tests.emptyText} />
{:else}
  <div class="test-results">
    {#each errors as error, index (index)}
      <div class="script-error">
        <p class="title">
          {strings.tests.scriptError(where(error.source))}
          {#if error.kind !== "exception"}
            <span class="detail">{strings.tests.kinds[error.kind]}</span>
          {/if}
          {#if error.line !== null}
            <span class="detail">{strings.tests.position(error.line, error.column)}</span>
          {/if}
        </p>
        <pre class="message">{error.message}</pre>
      </div>
    {/each}
    {#if tests.length > 0}
      <p class="summary">{strings.tests.summary(totals.passed, totals.failed, totals.skipped)}</p>
      <ul>
        {#each tests as test, index (index)}
          {@const state = outcome(test)}
          <li>
            <span class={["state", state]}>{strings.tests[state]}</span>
            <span class="name">{test.name}</span>
            {#if test.source.node_kind !== "request"}
              <span class="where">{where(test.source)}</span>
            {/if}
            {#if state === "failed" && test.error}
              <pre class="message">{test.error}</pre>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .test-results {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space) * 2);
  }

  .script-error {
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    background: var(--surface-2);
    border-left: 3px solid var(--danger);
    border-radius: var(--radius);
  }

  p {
    margin: 0;
  }

  .title {
    color: var(--danger);
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .detail {
    margin-left: calc(var(--space) * 2);
    color: var(--text-muted);
    font-weight: 400;
  }

  .summary {
    color: var(--text-muted);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: var(--space);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* The status and the name on one line; the text of the error takes a line of its own under them. */
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 calc(var(--space) * 2);
  }

  .state {
    flex: none;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
  }

  .state.passed {
    color: var(--status-ok);
  }

  .state.failed {
    color: var(--danger);
  }

  .state.skipped {
    color: var(--text-muted);
  }

  .name {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .where {
    color: var(--text-muted);
    font-size: 11px;
  }

  .message {
    flex-basis: 100%;
    margin: var(--space) 0 0;
    color: var(--text-muted);
    font-family: var(--font-mono);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
