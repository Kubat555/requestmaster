<script lang="ts">
  import { getContext, tick } from "svelte";
  import ConsoleView from "./ConsoleView.svelte";
  import CookiesView from "./CookiesView.svelte";
  import HeadersView from "./HeadersView.svelte";
  import ResponseBody from "./ResponseBody.svelte";
  import ResponseStatus from "./ResponseStatus.svelte";
  import SendWarnings from "./SendWarnings.svelte";
  import TestResults from "./TestResults.svelte";
  import EmptyState from "./ui/EmptyState.svelte";
  import Spinner from "./ui/Spinner.svelte";
  import type { Api } from "../lib/api";
  import { exportToFile } from "../lib/exporting";
  import { formatElapsed } from "../lib/format";
  import { icons } from "../lib/icons";
  import { motion } from "../lib/motion.svelte";
  import { playOnce } from "../lib/playOnce";
  import type { Sending } from "../lib/sending.svelte";
  import { slideIndicator } from "../lib/slideIndicator";
  import { strings } from "../lib/strings";
  import { toasts } from "../lib/toasts.svelte";
  import type { ResponseDto } from "../lib/types";

  interface Props {
    /** The send state of the tab whose response is shown. */
    sending: Sending;
    /** A new value shows the body of the response and opens the search in it. */
    findSignal?: number;
    /** The request whose response is shown: its collection gets the consent. Without it the warning has no button. */
    nodeId?: string;
  }

  let { sending, findSignal, nodeId }: Props = $props();

  const api = getContext<Api>("api");

  type Section = "body" | "headers" | "cookies" | "tests" | "console";
  const SECTIONS: Section[] = ["body", "headers", "cookies", "tests", "console"];
  /** A send that takes less does not replace what the panel shows with the waiting indicator. */
  const INDICATOR_DELAY_MS = 150;
  /** How often the time of the running send is written anew. */
  const ELAPSED_TICK_MS = 100;

  const id = $props.id();
  /** The section the user chose. Stays as chosen when the next response arrives, also over a result without it. */
  let section = $state<Section>("body");
  /** A send has been running for longer than the delay. */
  let waiting = $state(false);
  /** How long the running send has been going, ms: from its real start, not from when the indicator appeared. */
  let elapsed = $state(0);

  $effect(() => {
    if (sending.state !== "sending") return;
    // Read on every tick: a send started while another one runs counts from its own start.
    const measure = () => (elapsed = performance.now() - sending.startedAt);
    let ticker: ReturnType<typeof setInterval> | undefined;
    const timer = setTimeout(() => {
      measure();
      waiting = true;
      ticker = setInterval(measure, ELAPSED_TICK_MS);
    }, INDICATOR_DELAY_MS);
    return () => {
      clearTimeout(timer);
      clearInterval(ticker);
      waiting = false;
    };
  });

  const result = $derived(sending.result);
  const response = $derived(result?.response ?? null);
  const error = $derived(result?.error ?? null);
  const tests = $derived(result?.tests ?? []);
  const scriptErrors = $derived(result?.script_errors ?? []);
  const consoleLines = $derived(result?.console ?? []);

  /**
   * The sections there is something to show in: all of them for a response; without one — only what the
   * scripts left.
   */
  const available = $derived.by<Section[]>(() => {
    if (response) return SECTIONS;
    const sections: Section[] = [];
    if (tests.length > 0 || scriptErrors.length > 0) sections.push("tests");
    if (consoleLines.length > 0) sections.push("console");
    return sections;
  });
  /** The section on the screen: the chosen one, or the first there is while the chosen one has nothing to show. */
  const shown = $derived(available.includes(section) ? section : available[0]);

  let panel: HTMLDivElement;
  /** The response that has already arrived on the screen. */
  let arrived: ResponseDto | null = null;

  // The rich motion level: a new response rises into the panel, once.
  $effect(() => {
    if (!response || response === arrived) return;
    arrived = response;
    if (motion.rich) playOnce(panel, "arrived");
  });

  /**
   * The view of the body, chosen in `ResponseBody` and used here to copy what is shown. Every response starts
   * with Pretty when its body is JSON.
   */
  let pretty = $derived(response?.body.json ?? false);

  /**
   * Puts the body into the clipboard as it is shown; resolves to whether it is there. The text comes from the
   * core; a failure of the command is shown with its message, a clipboard that is missing or refuses — with a
   * message of its own.
   */
  async function copyBody(): Promise<boolean> {
    if (!response) return false;
    let text: string;
    try {
      text = await api.copyResponseBody(response.body.id, pretty);
    } catch (error) {
      toasts.error(String(error));
      return false;
    }
    try {
      await navigator.clipboard.writeText(text);
      toasts.info(strings.response.copied);
      return true;
    } catch {
      toasts.error(strings.response.copyFailed);
      return false;
    }
  }

  /** Writes the body, as received and unpacked, to a file the user chooses, and tells where the file is. */
  function saveBody() {
    if (!response) return;
    const bodyId = response.body.id;
    void exportToFile(() => api.saveResponseBody(bodyId));
  }

  /** Lets the scripts of the collection of `id` run; the send that was warned about stays as it is. */
  async function allowScripts(id: string) {
    try {
      await api.setScriptsAllowed(id, true);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    toasts.info(strings.response.scriptsAllowedNow);
  }

  /** The view of the body on the screen. */
  let bodyView = $state<ResponseBody>();
  /** The value of `findSignal` that was answered last. */
  let answeredFind: number | undefined;

  $effect(() => {
    if (findSignal === undefined || findSignal === answeredFind) return;
    answeredFind = findSignal;
    void openSearch();
  });

  /** Shows the body and opens the search in it; a response without a body has nothing to search in. */
  async function openSearch() {
    if (!response || response.body.total === 0) return;
    section = "body";
    // The search field takes the focus, and it cannot while the body is still hidden.
    await tick();
    bodyView?.openSearch();
  }

  /** Number of rows next to the name of a section; 0 — no number. The tests have a mark of their own. */
  const counts = $derived<Record<Section, number>>({
    body: 0,
    headers: response?.headers.length ?? 0,
    cookies: response?.cookies.length ?? 0,
    tests: 0,
    console: consoleLines.length,
  });

  /**
   * The mark of the tests: how many of those that ran passed, or `!` for a script error without a single
   * test; `null` — nothing to mark. A skipped test did not run. One failed test or one script error makes the
   * mark bad.
   */
  const testsMark = $derived.by(() => {
    if (tests.length === 0 && scriptErrors.length === 0) return null;
    if (tests.length === 0) return { text: "!", ok: false };
    const ran = tests.filter((test) => !test.skipped);
    const passed = ran.filter((test) => test.passed).length;
    return { text: `${passed}/${ran.length}`, ok: passed === ran.length && scriptErrors.length === 0 };
  });

  /** What went wrong, by the kind of the error; the fields whose files are missing are named. */
  const errorText = $derived.by(() => {
    if (!error) return "";
    const text = strings.response.errors[error.kind];
    return error.kind === "unconfirmed_files" ? `${text}: ${error.fields.join(", ")}` : text;
  });
</script>

<div bind:this={panel} class="response-panel">
  {#if waiting}
    <!-- What happens is said by the name; the ring, the line and the time are for the eyes, and the time,
         changing ten times a second, is kept from a screen reader. At the rich motion level the ring gives
         way to the two arrows of the application, which bend into a ring and circle one after the other. -->
    <div class="sending" role="status" aria-label={strings.response.sending}>
      <div class="progress" aria-hidden="true"></div>
      {#if motion.rich}
        <svg
          class="exchange"
          width="32"
          height="32"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          stroke-width="1.6"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <g class="straight">
            {#each icons.send as d, index (index)}
              <path {d} />
            {/each}
          </g>
          <!-- The same two arrows bent into a ring: each is an arc of the circle with its head at the end. -->
          <g class="ring">
            <path d="M3.3 6.29A5 5 0 0 1 12.7 6.29M10.57 4.8L12.7 6.29L13.37 3.78" />
            <path d="M12.7 9.71A5 5 0 0 1 3.3 9.71M5.43 11.2L3.3 9.71L2.63 12.22" />
          </g>
        </svg>
      {:else}
        <Spinner size="lg" />
      {/if}
      <span class="elapsed" aria-hidden="true">{formatElapsed(elapsed)}</span>
    </div>
  {:else if !result}
    <EmptyState icon="send" title={strings.response.emptyTitle} text={strings.response.empty} />
  {:else}
    <SendWarnings warnings={result.warnings} onallow={nodeId === undefined ? undefined : () => allowScripts(nodeId)} />
    {#if error}
      <div class="failure">
        <!-- A cancel is what the user asked for and a skip is what a script asked for, so neither is shown as
             an error. -->
        <p class="reason" class:error={error.kind !== "cancelled" && error.kind !== "skipped"}>{errorText}</p>
        {#if error.message}
          <pre class="message">{error.message}</pre>
        {/if}
      </div>
    {:else if response}
      <ResponseStatus {response} oncopy={copyBody} onsave={saveBody} />
    {/if}

    <!-- Without a response there are sections only for what the scripts left; the reason stays above them. -->
    {#if available.length > 0}
      <div class="section-tabs" role="tablist" aria-label={strings.response.sections} {@attach slideIndicator}>
        {#each available as name (name)}
          <button
            type="button"
            role="tab"
            id="{id}-tab-{name}"
            aria-selected={shown === name}
            aria-controls="{id}-panel-{name}"
            onclick={() => (section = name)}
          >
            {strings.response[name]}
            {#if name === "tests" && testsMark}
              <span class={["count", testsMark.ok ? "ok" : "bad"]}>{testsMark.text}</span>
            {:else if counts[name] > 0}
              <span class="count">{counts[name]}</span>
            {/if}
          </button>
        {/each}
      </div>

      {#if response}
        <!-- The body stays while another section is shown, so that it is not fetched again. -->
        <div class="panel" role="tabpanel" id="{id}-panel-body" aria-labelledby="{id}-tab-body" hidden={shown !== "body"}>
          {#key response.body.id}
            <ResponseBody bind:this={bodyView} body={response.body} decodeError={response.decode_error} bind:pretty onsave={saveBody} />
          {/key}
        </div>
      {/if}
      {#if shown === "headers" && response}
        <div class="panel scroll" role="tabpanel" id="{id}-panel-headers" aria-labelledby="{id}-tab-headers">
          <HeadersView headers={response.headers} />
        </div>
      {:else if shown === "cookies" && response}
        <div class="panel scroll" role="tabpanel" id="{id}-panel-cookies" aria-labelledby="{id}-tab-cookies">
          <CookiesView cookies={response.cookies} />
        </div>
      {:else if shown === "tests"}
        <div class="panel scroll" role="tabpanel" id="{id}-panel-tests" aria-labelledby="{id}-tab-tests">
          <TestResults {tests} errors={scriptErrors} />
        </div>
      {:else if shown === "console"}
        <div class="panel scroll" role="tabpanel" id="{id}-panel-console" aria-labelledby="{id}-tab-console">
          <ConsoleView lines={consoleLines} truncated={result.console_truncated} />
        </div>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .response-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  /* The ring and the time under it, in the middle of the panel; the ring takes the colour of the accent. */
  .sending {
    position: relative;
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: calc(var(--space) * 2);
    min-height: 0;
    overflow: hidden;
    color: var(--accent);
  }

  .elapsed {
    color: var(--text-muted);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  /* A thin line along the top edge of the panel; a piece of it runs across again and again. */
  .progress {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 2px;
    overflow: hidden;
  }

  .progress::before {
    content: "";
    display: block;
    width: 40%;
    height: 100%;
    background: currentColor;
    animation: progress-run var(--dur-loop) ease-in-out infinite var(--loop-state);
  }

  /* From behind the left edge to behind the right one: 250% of its own width is the whole line. */
  @keyframes progress-run {
    from {
      transform: translateX(-100%);
    }

    to {
      transform: translateX(250%);
    }
  }

  /* Without motion the line stands still along the whole edge; the time goes on. */
  :global(:root[data-motion="off"]) .progress::before {
    width: 100%;
    animation: none;
  }

  /* The rich motion level: the two straight arrows, pointing opposite ways, bend into a ring — they turn and
     fade while the bent ones come in already turning — and then the ring goes round, one arrow chasing the
     other, for as long as the request is on its way. Nothing is cut at the edge of the drawing. */
  :global(:root[data-motion="rich"]) .exchange {
    overflow: visible;
  }

  :global(:root[data-motion="rich"]) .exchange g {
    transform-box: view-box;
    transform-origin: center;
  }

  :global(:root[data-motion="rich"]) .exchange .straight {
    animation: bend-out var(--dur-bend) var(--ease) var(--dur-4) both;
  }

  :global(:root[data-motion="rich"]) .exchange .ring {
    opacity: 0;
    animation:
      bend-in var(--dur-bend) var(--ease) var(--dur-4) forwards,
      chase var(--dur-loop) linear var(--dur-4) infinite var(--loop-state);
  }

  @keyframes bend-out {
    to {
      opacity: 0;
      transform: rotate(120deg);
    }
  }

  @keyframes bend-in {
    to {
      opacity: 1;
    }
  }

  @keyframes chase {
    from {
      transform: rotate(0deg);
    }

    to {
      transform: rotate(360deg);
    }
  }

  /* The rich motion level: a new response fades in while its parts rise into their places. The parts are cut
     at the edge of the panel meanwhile, so that nothing sticks out of it and no scrollbar blinks. The fade of
     a shown section (base.css) stays in the list of a part, so that it is not played again after the rise. */
  :global(:root[data-motion="rich"]) .response-panel:global(.arrived) {
    overflow: hidden;
    animation: fade-in var(--dur-4) var(--ease);
  }

  :global(:root[data-motion="rich"]) .response-panel:global(.arrived) > :global(*) {
    animation:
      fade-in var(--dur-3) var(--ease),
      arrive var(--dur-4) var(--ease);
  }

  @keyframes arrive {
    from {
      transform: translateY(8px);
    }
  }

  /* Takes what it needs, up to a part of the height: the sections under it stay on the screen. */
  .failure {
    flex: none;
    max-height: 40%;
    padding: calc(var(--space) * 3);
    overflow: auto;
  }

  .reason {
    margin: 0;
    font-weight: 600;
  }

  .reason.error {
    color: var(--danger);
  }

  .message {
    margin: calc(var(--space) * 2) 0 0;
    color: var(--text-muted);
    font-family: var(--font-mono);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .panel {
    flex: 1;
    min-height: 0;
  }

  .scroll {
    padding: calc(var(--space) * 2);
    overflow: auto;
  }
</style>
