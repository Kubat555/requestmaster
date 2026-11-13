<script lang="ts">
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import type { IconName } from "../../lib/icons";

  interface Props {
    icon: IconName;
    /** What is missing, in a few words. */
    title: string;
    /** What to do about it. */
    text?: string;
    /** Text of the button; without it, or without `onaction`, there is no button. */
    actionLabel?: string;
    onaction?: () => void;
  }

  let { icon, title, text, actionLabel, onaction }: Props = $props();
</script>

<!-- What a part of the window shows while it has nothing of its own to show. -->
<div class="empty-state">
  <span class="mark"><Icon name={icon} /></span>
  <p class="title">{title}</p>
  {#if text}
    <p class="text">{text}</p>
  {/if}
  {#if actionLabel && onaction}
    <div class="action">
      <Button onclick={onaction}>{actionLabel}</Button>
    </div>
  {/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: calc(var(--space) * 2);
    padding: calc(var(--space) * 8) calc(var(--space) * 4);
    text-align: center;
  }

  .mark {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    background: var(--surface-2);
    border-radius: 50%;
    color: var(--text-muted);
  }

  p {
    margin: 0;
    max-width: 320px;
  }

  .title {
    font-weight: 600;
  }

  .text {
    color: var(--text-muted);
  }

  .action {
    margin-top: calc(var(--space) * 2);
  }
</style>
