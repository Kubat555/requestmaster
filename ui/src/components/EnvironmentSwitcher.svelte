<script lang="ts">
  import { getContext } from "svelte";
  import Button from "./ui/Button.svelte";
  import Icon from "./ui/Icon.svelte";
  import Menu, { type MenuItem } from "./ui/Menu.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import type { Workspace } from "../lib/workspace.svelte";

  interface Props {
    workspace: Workspace;
  }

  let { workspace }: Props = $props();

  const api = getContext<Api>("api");

  /** The frame of the button: the menu appears under it. */
  let anchor = $state<HTMLSpanElement>();
  let open = $state(false);

  const active = $derived(workspace.environments.find((environment) => environment.id === workspace.activeEnvironmentId) ?? null);
  const name = $derived(active?.name ?? strings.environment.none);

  /** "No environment" first, then every environment. */
  const items = $derived<MenuItem[]>([
    { label: strings.environment.none, onselect: () => void workspace.setEnvironment(api, null) },
    ...workspace.environments.map((environment) => ({
      label: environment.name,
      onselect: () => void workspace.setEnvironment(api, environment.id),
    })),
  ]);
</script>

<!-- The button names the active environment; the dot is lit while one is chosen. -->
<span bind:this={anchor} class="switcher">
  <Button aria-haspopup="menu" aria-expanded={open} aria-label="{strings.environment.label}: {name}" onclick={() => (open = !open)}>
    <span class="dot" class:on={active !== null}></span>
    <span class="name">{name}</span>
    <Icon name="chevron-down" />
  </Button>
</span>
{#if open && anchor}
  <Menu label={strings.environment.label} {items} {anchor} onclose={() => (open = false)} />
{/if}

<style>
  .switcher {
    flex: none;
    display: flex;
    margin: 0 calc(var(--space) * 2);
  }

  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    background: var(--text-muted);
    border-radius: 50%;
  }

  .dot.on {
    background: var(--status-ok);
  }

  .name {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
