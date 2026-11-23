<script lang="ts">
  import { getContext, onMount, tick } from "svelte";
  import Dialog from "./Dialog.svelte";
  import Button from "./ui/Button.svelte";
  import Field from "./ui/Field.svelte";
  import Segmented from "./ui/Segmented.svelte";
  import type { Api } from "../lib/api";
  import { motion, motionLevelOf } from "../lib/motion.svelte";
  import { playOnce } from "../lib/playOnce";
  import { strings } from "../lib/strings";
  import { toasts } from "../lib/toasts.svelte";
  import { MOTION_LEVELS, type MotionLevel, type ProxyDto } from "../lib/types";

  interface Props {
    /** Called when the dialog is done: the settings are saved, or the user gave up. The owner removes it. */
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  const api = getContext<Api>("api");
  const id = $props.id();

  /** The longest timeout the core takes, in milliseconds: one hour. */
  const MAX_TIMEOUT_MS = 3_600_000;
  /** A timeout the field takes: a whole number without a sign. */
  const WHOLE = /^\d+$/;
  /** A proxy address the field takes: `http://` or `https://` with something after it. */
  const PROXY_URL = /^https?:\/\/\S/i;
  /** The motion levels to choose from. */
  const MOTION_OPTIONS = MOTION_LEVELS.map((value) => ({ value, label: strings.settings.motionLevels[value] }));

  /** The saved settings are read: the fields are on the screen. */
  let loaded = $state(false);
  let timeout = $state("");
  let verifyTls = $state(true);
  let proxyMode = $state<ProxyDto["mode"]>("system");
  /** The address of the manual proxy; kept while another choice is made, sent only with the manual one. */
  let proxyUrl = $state("");
  /** How much the window animates; applied when the settings are saved. */
  let motionLevel = $state<MotionLevel>("normal");
  /** The two settings of the scripts as they were read: the dialog has no fields for them and saves them back. */
  let scriptsEnabled = $state(true);
  let scriptTimeoutMs = $state(10000);

  let form: HTMLFormElement;

  /** The rich motion level: the fields whose values the save refused shake once. */
  async function shakeInvalid() {
    // The marks of the refused fields are on the screen after the next draw.
    await tick();
    for (const field of form.querySelectorAll('[aria-invalid="true"]')) {
      playOnce(field.closest("[data-pulse-frame]") ?? field, "shake");
    }
  }

  /** The last save found the value of the field unfit. */
  let timeoutInvalid = $state(false);
  let proxyUrlInvalid = $state(false);
  /** What the core answered when it could not read or save the settings; `null` — nothing. */
  let failure = $state<string | null>(null);
  let saving = $state(false);

  onMount(() => {
    api.getSettings().then(
      (settings) => {
        timeout = String(settings.timeout_ms);
        verifyTls = settings.verify_tls;
        proxyMode = settings.proxy.mode;
        proxyUrl = settings.proxy.mode === "manual" ? settings.proxy.url : "";
        motionLevel = motionLevelOf(settings.motion);
        scriptsEnabled = settings.scripts_enabled;
        scriptTimeoutMs = settings.script_timeout_ms;
        loaded = true;
      },
      (error) => (failure = String(error)),
    );
  });

  /** The timeout that is typed; `null` — not a whole number from 0 to the maximum. */
  function typedTimeout(): number | null {
    const text = timeout.trim();
    if (!WHOLE.test(text)) return null;
    const value = Number(text);
    return value <= MAX_TIMEOUT_MS ? value : null;
  }

  /** The proxy that is chosen; `null` — the manual one with an address that cannot be one. */
  function chosenProxy(): ProxyDto | null {
    if (proxyMode !== "manual") return { mode: proxyMode };
    const url = proxyUrl.trim();
    return PROXY_URL.test(url) ? { mode: "manual", url } : null;
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    const timeoutMs = typedTimeout();
    const proxy = chosenProxy();
    timeoutInvalid = timeoutMs === null;
    proxyUrlInvalid = proxy === null;
    failure = null;
    if (timeoutMs === null || proxy === null) {
      if (motion.rich) void shakeInvalid();
      return;
    }

    saving = true;
    try {
      await api.saveSettings({
        timeout_ms: timeoutMs,
        verify_tls: verifyTls,
        proxy,
        motion: motionLevel,
        scripts_enabled: scriptsEnabled,
        script_timeout_ms: scriptTimeoutMs,
      });
    } catch (error) {
      failure = String(error);
      return;
    } finally {
      saving = false;
    }
    // The window moves the new way from now on; a send has nothing to do with it.
    motion.choose(motionLevel);
    toasts.info(strings.settings.saved);
    onclose();
  }
</script>

<!-- The application settings. Nothing is saved until Save: Cancel and Escape leave them as they were. -->
<Dialog title={strings.settings.title} {onclose}>
  <form bind:this={form} class="settings" novalidate onsubmit={save}>
    {#if loaded}
      <div class="row">
        <span class="name">{strings.settings.timeout}</span>
        <Field
          mono
          label={strings.settings.timeout}
          inputmode="numeric"
          autocomplete="off"
          spellcheck="false"
          bind:value={timeout}
          aria-invalid={timeoutInvalid ? "true" : undefined}
          aria-describedby="{id}-timeout-note"
        />
        <span class="note" id="{id}-timeout-note">
          {#if timeoutInvalid}
            <span class="invalid">{strings.settings.timeoutInvalid}</span>
          {:else}
            {strings.settings.timeoutNote}
          {/if}
        </span>
      </div>

      <div class="row">
        <label class="choice">
          <input type="checkbox" bind:checked={verifyTls} aria-describedby="{id}-tls-note" />
          {strings.settings.verifyTls}
        </label>
        <span class="note" id="{id}-tls-note">{strings.settings.verifyTlsNote}</span>
      </div>

      <fieldset class="row">
        <legend class="name">{strings.settings.proxy}</legend>
        <label class="choice">
          <input type="radio" name="{id}-proxy" value="system" bind:group={proxyMode} aria-describedby="{id}-system-note" />
          {strings.settings.proxySystem}
        </label>
        <span class="note indented" id="{id}-system-note">{strings.settings.proxySystemNote}</span>
        <label class="choice">
          <input type="radio" name="{id}-proxy" value="off" bind:group={proxyMode} />
          {strings.settings.proxyOff}
        </label>
        <label class="choice">
          <input type="radio" name="{id}-proxy" value="manual" bind:group={proxyMode} />
          {strings.settings.proxyManual}
        </label>
        {#if proxyMode === "manual"}
          <Field
            mono
            label={strings.settings.proxyUrl}
            placeholder={strings.settings.proxyUrlPlaceholder}
            autocomplete="off"
            spellcheck="false"
            bind:value={proxyUrl}
            aria-invalid={proxyUrlInvalid ? "true" : undefined}
            aria-describedby={proxyUrlInvalid ? `${id}-proxy-note` : undefined}
          />
          {#if proxyUrlInvalid}
            <span class="note invalid" id="{id}-proxy-note">{strings.settings.proxyUrlInvalid}</span>
          {/if}
        {/if}
      </fieldset>

      <div class="row">
        <span class="name">{strings.settings.motion}</span>
        <Segmented options={MOTION_OPTIONS} bind:value={motionLevel} label={strings.settings.motion} />
        <span class="note">{strings.settings.motionNote}</span>
      </div>
    {/if}

    {#if failure !== null}
      <p class="failure" role="alert">{failure}</p>
    {/if}

    <div class="dialog-actions">
      {#if loaded}
        <Button type="submit" variant="primary" disabled={saving}>{strings.settings.save}</Button>
      {/if}
      <Button onclick={onclose}>{strings.settings.cancel}</Button>
    </div>
  </form>
</Dialog>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space) * 4);
  }

  .row {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: calc(var(--space) * 1.5);
    min-width: 0;
    margin: 0;
    padding: 0;
    border: none;
  }

  legend.name {
    padding: 0;
    margin-bottom: calc(var(--space) * 1.5);
  }

  .choice {
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 2);
  }

  .choice input {
    margin: 0;
    accent-color: var(--accent);
  }

  .note {
    color: var(--text-muted);
  }

  /* Under the text of its choice, not under the mark. */
  .note.indented {
    padding-left: calc(var(--space) * 6);
  }

  .invalid {
    color: var(--danger);
  }

  .failure {
    margin: 0;
    color: var(--danger);
    overflow-wrap: anywhere;
  }
</style>
