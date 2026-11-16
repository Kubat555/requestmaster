<script lang="ts">
  import Field from "./ui/Field.svelte";
  import { strings } from "../lib/strings";
  import type { RequestSettingsDraft } from "../lib/types";

  interface Props {
    /** The settings of a request, edited in place; `null` — the setting is not set and the default applies. */
    settings: RequestSettingsDraft;
  }

  let { settings = $bindable() }: Props = $props();

  type Switch = "follow_redirects" | "strict_ssl" | "disable_body_pruning";

  /** The choices of a switch and the value each one stands for. */
  const CHOICES = [
    { value: "default", label: strings.requestSettings.byDefault, setting: null },
    { value: "true", label: strings.requestSettings.yes, setting: true },
    { value: "false", label: strings.requestSettings.no, setting: false },
  ] as const;

  /** A maximum of redirects the field takes: a whole number from 0 to 9999. */
  const MAX_REDIRECTS = /^\d{1,4}$/;

  const id = $props.id();
  /** What is typed in the field of the maximum while it is not a number the draft can take; `null` — nothing. */
  let rejected = $state<string | null>(null);

  function choice(setting: boolean | null): string {
    return CHOICES.find((candidate) => candidate.setting === setting)!.value;
  }

  function onChosen(name: Switch, value: string) {
    settings[name] = CHOICES.find((candidate) => candidate.value === value)?.setting ?? null;
  }

  function onMaxTyped(text: string) {
    if (text === "") {
      settings.max_redirects = null;
    } else if (MAX_REDIRECTS.test(text)) {
      settings.max_redirects = Number(text);
    } else {
      // The draft keeps its value: the core would refuse anything but a whole number.
      rejected = text;
      return;
    }
    rejected = null;
  }
</script>

{#snippet switchRow(name: Switch, label: string, note: string)}
  <span class="name">{label}</span>
  <select
    class="select"
    aria-label={label}
    value={choice(settings[name])}
    onchange={(event) => onChosen(name, event.currentTarget.value)}
  >
    {#each CHOICES as option (option.value)}
      <option value={option.value}>{option.label}</option>
    {/each}
  </select>
  <span class="note">{note}</span>
{/snippet}

<!-- The settings of one request, a row each: the name, the choice, what happens when nothing is chosen. -->
<div class="settings-editor">
  {@render switchRow("follow_redirects", strings.requestSettings.followRedirects, strings.requestSettings.followRedirectsNote)}

  <span class="name">{strings.requestSettings.maxRedirects}</span>
  <Field
    mono
    label={strings.requestSettings.maxRedirects}
    inputmode="numeric"
    autocomplete="off"
    spellcheck="false"
    value={rejected ?? (settings.max_redirects === null ? "" : String(settings.max_redirects))}
    aria-invalid={rejected === null ? undefined : "true"}
    aria-describedby="{id}-max-note"
    oninput={(event) => onMaxTyped(event.currentTarget.value)}
  />
  <span class="note" id="{id}-max-note">
    {#if rejected === null}
      {strings.requestSettings.maxRedirectsNote}
    {:else}
      <span class="invalid">{strings.requestSettings.maxRedirectsInvalid}</span>
    {/if}
  </span>

  {@render switchRow("strict_ssl", strings.requestSettings.strictSsl, strings.requestSettings.strictSslNote)}
  <!-- The setting is stored the other way round: `true` keeps the body of a method that normally has none. -->
  {@render switchRow("disable_body_pruning", strings.requestSettings.sendBody, strings.requestSettings.sendBodyNote)}
</div>

<style>
  .settings-editor {
    display: grid;
    grid-template-columns: minmax(160px, 280px) 160px minmax(0, 1fr);
    align-items: center;
    gap: calc(var(--space) * 2) calc(var(--space) * 3);
    padding: calc(var(--space) * 2) var(--space);
  }

  .note {
    color: var(--text-muted);
  }

  .invalid {
    color: var(--danger);
  }
</style>
