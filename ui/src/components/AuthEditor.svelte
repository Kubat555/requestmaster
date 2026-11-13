<script lang="ts">
  import Button from "./ui/Button.svelte";
  import Field from "./ui/Field.svelte";
  import { strings } from "../lib/strings";
  import type { ApiKeyPlacement, Auth } from "../lib/types";

  interface Props {
    /** Authorization of the node, edited in place or replaced when its type changes; `null` — not set. */
    auth: Auth | null;
    /**
     * Whether the node has a parent to inherit from. With one, `null` is "inherit" and "no authorization" is
     * `no_auth`. Without one (a collection) there is nothing to inherit: `null` is "no authorization", and a
     * stored `no_auth` is shown the same way.
     */
    inherit: boolean;
  }

  let { auth = $bindable(), inherit }: Props = $props();

  /** Value of the option "inherit"; never a type of `Auth`. */
  const INHERIT = "inherit";
  const PLACEMENTS: ApiKeyPlacement[] = ["header", "query"];
  const PLACEMENT_NAMES = { header: strings.auth.inHeader, query: strings.auth.inQuery };

  const selected = $derived(auth?.type ?? (inherit ? INHERIT : "no_auth"));
  /** Labels of the masked fields the user asked to show. */
  let revealed = $state<string[]>([]);

  function onTypeChosen(event: Event) {
    const type = (event.currentTarget as HTMLSelectElement).value;
    revealed = [];
    switch (type) {
      case "no_auth":
        auth = inherit ? { type: "no_auth" } : null;
        break;
      case "basic":
        auth = { type: "basic", username: "", password: "" };
        break;
      case "bearer":
        auth = { type: "bearer", token: "" };
        break;
      case "api_key":
        auth = { type: "api_key", key: "", value: "", placement: "header" };
        break;
      case "oauth2_token":
        auth = { type: "oauth2_token", token: "", header_prefix: "Bearer", placement: "header" };
        break;
      default:
        auth = null;
    }
  }

  function toggleRevealed(label: string) {
    revealed = revealed.includes(label) ? revealed.filter((other) => other !== label) : [...revealed, label];
  }
</script>

<!-- A field that holds a password or a token: masked, with a button that shows it. -->
{#snippet secretField(label: string, value: string, set: (value: string) => void)}
  {@const shown = revealed.includes(label)}
  <div class="field">
    <span>{label}</span>
    <span class="secret">
      <Field
        mono
        type={shown ? "text" : "password"}
        {label}
        spellcheck="false"
        autocomplete="off"
        {value}
        oninput={(event) => set(event.currentTarget.value)}
      />
      <Button
        aria-pressed={shown}
        aria-label="{shown ? strings.table.hide : strings.table.show}: {label}"
        onclick={() => toggleRevealed(label)}
      >
        {shown ? strings.table.hide : strings.table.show}
      </Button>
    </span>
  </div>
{/snippet}

{#snippet placementField(value: ApiKeyPlacement, set: (value: ApiKeyPlacement) => void)}
  <div class="field">
    <span>{strings.auth.placement}</span>
    <select class="select" aria-label={strings.auth.placement} {value} onchange={(event) => set(event.currentTarget.value as ApiKeyPlacement)}>
      {#each PLACEMENTS as placement (placement)}
        <option value={placement}>{PLACEMENT_NAMES[placement]}</option>
      {/each}
    </select>
  </div>
{/snippet}

<div class="auth-editor">
  <div class="field">
    <span>{strings.auth.type}</span>
    <select class="select" aria-label={strings.auth.type} value={selected} onchange={onTypeChosen}>
      {#if inherit}
        <option value={INHERIT}>{strings.auth.inherit}</option>
      {/if}
      <option value="no_auth">{strings.auth.none}</option>
      <option value="basic">{strings.auth.basic}</option>
      <option value="bearer">{strings.auth.bearer}</option>
      <option value="api_key">{strings.auth.apiKey}</option>
      <option value="oauth2_token">{strings.auth.oauth2}</option>
      {#if auth?.type === "unsupported"}
        <!-- Only while it is the stored type: once another one is chosen, there is no way back to it. -->
        <option value="unsupported">{auth.auth_type} — {strings.auth.unsupported}</option>
      {/if}
    </select>
  </div>

  {#if auth === null}
    {#if inherit}
      <p class="note">{strings.auth.inheritNote}</p>
    {/if}
  {:else if auth.type === "basic"}
    {@const basic = auth}
    <div class="field">
      <span>{strings.auth.username}</span>
      <Field mono label={strings.auth.username} spellcheck="false" autocomplete="off" bind:value={basic.username} />
    </div>
    {@render secretField(strings.auth.password, basic.password, (value) => (basic.password = value))}
  {:else if auth.type === "bearer"}
    {@const bearer = auth}
    {@render secretField(strings.auth.token, bearer.token, (value) => (bearer.token = value))}
  {:else if auth.type === "api_key"}
    {@const apiKey = auth}
    <div class="field">
      <span>{strings.auth.key}</span>
      <Field mono label={strings.auth.key} spellcheck="false" bind:value={apiKey.key} />
    </div>
    <div class="field">
      <span>{strings.auth.value}</span>
      <Field mono label={strings.auth.value} spellcheck="false" autocomplete="off" bind:value={apiKey.value} />
    </div>
    {@render placementField(apiKey.placement, (value) => (apiKey.placement = value))}
  {:else if auth.type === "oauth2_token"}
    {@const oauth = auth}
    {@render secretField(strings.auth.token, oauth.token, (value) => (oauth.token = value))}
    <div class="field">
      <span>{strings.auth.prefix}</span>
      <Field mono label={strings.auth.prefix} spellcheck="false" bind:value={oauth.header_prefix} />
    </div>
    {@render placementField(oauth.placement, (value) => (oauth.placement = value))}
  {:else if auth.type === "unsupported"}
    <p class="note">{strings.auth.unsupportedNote}</p>
  {/if}
</div>

<style>
  .auth-editor {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space) * 2);
    max-width: 560px;
    padding: calc(var(--space) * 2) var(--space);
  }

  .field {
    display: grid;
    grid-template-columns: 160px minmax(0, 1fr);
    align-items: center;
    gap: calc(var(--space) * 2);
  }

  .field > span:first-child {
    color: var(--text-muted);
  }

  .secret {
    display: flex;
    gap: calc(var(--space) * 2);
  }

  .note {
    margin: 0;
    color: var(--text-muted);
  }
</style>
