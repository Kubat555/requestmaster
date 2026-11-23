<script lang="ts">
  import CodeEditor from "./CodeEditor.svelte";
  import Segmented from "./ui/Segmented.svelte";
  import { strings } from "../lib/strings";
  import type { ScriptsDraft } from "../lib/types";

  interface Props {
    /** The scripts of a node, edited in place. */
    scripts: ScriptsDraft;
  }

  let { scripts = $bindable() }: Props = $props();

  type Kind = keyof ScriptsDraft;
  const KINDS: { value: Kind; label: string }[] = [
    { value: "pre_request", label: strings.scripts.kinds.pre_request },
    { value: "test", label: strings.scripts.kinds.test },
  ];

  /** The script on the screen. */
  let kind = $state<Kind>("pre_request");
</script>

<!-- The two scripts of a node. -->
<div class="scripts-editor">
  <div class="bar">
    <Segmented options={KINDS} bind:value={kind} label={strings.scripts.kind} />
  </div>
  <div class="code">
    <!-- A new editor for the other script: in one editor Ctrl+Z would bring the text of the script left
         into the script shown. -->
    {#key kind}
      <CodeEditor
        value={scripts[kind]}
        language="javascript"
        completions="script"
        label={strings.scripts.text}
        onchange={(text) => (scripts[kind] = text)}
      />
    {/key}
  </div>
</div>

<style>
  .scripts-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar {
    flex: none;
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 3);
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
  }

  .code {
    flex: 1;
    min-height: 120px;
    border-top: 1px solid var(--border);
  }

  /* The rich motion level: the editor of the script that was chosen fades in. */
  :global(:root[data-motion="rich"]) .code > :global(*) {
    animation: fade-in var(--dur-3) var(--ease);
  }
</style>
