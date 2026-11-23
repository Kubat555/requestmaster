<script lang="ts" module>
  import { strings } from "../lib/strings";

  /** Everything of CodeMirror the editor uses. Loaded once, when the first editor is shown. */
  let loading: ReturnType<typeof load> | undefined;
  /** The JavaScript language: a chunk of its own, loaded once, by the first editor that shows JavaScript. */
  let loadingJavascript: Promise<typeof import("@codemirror/lang-javascript")> | undefined;

  async function load() {
    const [
      { EditorView },
      { Compartment, EditorState },
      { json },
      { HighlightStyle, syntaxHighlighting },
      { tags },
      { variableSupport },
      { openSearchPanel },
      { baseSetup, indentation, tabIndentation },
      { contentReplaced, typingGlow },
      { scriptCompletions },
    ] = await Promise.all([
      import("@codemirror/view"),
      import("@codemirror/state"),
      import("@codemirror/lang-json"),
      import("@codemirror/language"),
      import("@lezer/highlight"),
      import("../lib/codemirror/variables"),
      import("@codemirror/search"),
      import("../lib/codemirror/setup"),
      import("../lib/codemirror/typingGlow"),
      import("../lib/codemirror/scriptCompletions"),
    ]);

    // Colours come from tokens.css. The built-in highlighting is made for a light background.
    const theme = EditorView.theme(
      {
        "&": { height: "100%", color: "var(--text)", backgroundColor: "var(--bg)", fontSize: "var(--font-size)" },
        "&.cm-focused": { outline: "2px solid var(--accent)", outlineOffset: "-2px" },
        ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.5" },
        ".cm-content": { caretColor: "var(--accent)", outline: "none" },
        ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)" },
        ".cm-gutters": { backgroundColor: "var(--bg)", color: "var(--code-line)", borderRight: "1px solid var(--border)" },
        // Translucent: the selection is drawn under the lines and must stay visible on the active one.
        ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "color-mix(in srgb, var(--surface-2) 60%, transparent)" },
        "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
          { backgroundColor: "color-mix(in srgb, var(--accent) 30%, transparent)" },
        ".cm-panels, .cm-tooltip": { backgroundColor: "var(--surface-2)", color: "var(--text)", border: "1px solid var(--border)" },
        // The chosen line of the list of the autocompletion; the selector outweighs the one of the library.
        "&.cm-editor .cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
          backgroundColor: "var(--surface)",
          color: "var(--text)",
        },
        ".cm-foldPlaceholder": { backgroundColor: "var(--surface-2)", color: "var(--text-muted)", border: "none" },
        // The chevron of a block that can be folded: in the middle of the height of its line.
        ".cm-fold-marker": { display: "flex", alignItems: "center", height: "100%", color: "var(--text-muted)" },
        ".cm-fold-marker:hover": { color: "var(--text)" },
        // The search panel: its fields and buttons look like the ones of the window. The selectors outweigh
        // the ones of the library.
        "& .cm-panel.cm-search": { padding: "var(--space) calc(var(--space) * 2)", fontFamily: "var(--font-ui)" },
        "& .cm-panel.cm-search label": { fontSize: "var(--font-size)", color: "var(--text-muted)" },
        "& .cm-panel.cm-search input[type=checkbox]": { accentColor: "var(--accent)" },
        "& .cm-textfield": {
          height: "var(--control-height-sm)",
          padding: "0 calc(var(--space) * 2)",
          backgroundColor: "var(--bg)",
          border: "1px solid var(--border)",
          borderRadius: "var(--radius)",
          color: "var(--text)",
          fontSize: "var(--font-size)",
        },
        "& .cm-button": {
          height: "var(--control-height-sm)",
          padding: "0 calc(var(--space) * 2)",
          backgroundColor: "var(--surface)",
          backgroundImage: "none",
          border: "1px solid var(--border)",
          borderRadius: "var(--radius)",
          color: "var(--text)",
          fontSize: "var(--font-size)",
          cursor: "pointer",
        },
        "& .cm-button:hover": { borderColor: "var(--text-muted)" },
        "& .cm-button:active": { backgroundImage: "none", backgroundColor: "var(--surface-2)" },
        "& .cm-panel.cm-search [name=close]": { color: "var(--text-muted)", cursor: "pointer" },
        "& .cm-panel.cm-search [name=close]:hover": { color: "var(--text)" },
        "& .cm-textfield:focus-visible, & .cm-button:focus-visible, & .cm-panel.cm-search [name=close]:focus-visible": {
          outline: "2px solid var(--accent)",
          outlineOffset: "-1px",
        },
        "& .cm-searchMatch": { backgroundColor: "color-mix(in srgb, var(--status-redirect) 35%, transparent)" },
        "& .cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "color-mix(in srgb, var(--accent) 55%, transparent)" },
      },
      { dark: true },
    );
    const highlight = syntaxHighlighting(
      HighlightStyle.define([
        { tag: tags.propertyName, color: "var(--code-key)" },
        { tag: tags.string, color: "var(--code-string)" },
        { tag: tags.number, color: "var(--code-number)" },
        { tag: [tags.bool, tags.null, tags.keyword], color: "var(--code-atom)" },
        { tag: tags.comment, color: "var(--text-muted)" },
        { tag: tags.invalid, color: "var(--danger)" },
      ]),
    );

    // The texts of the search panel, by the phrases of the library.
    const text = strings.search;
    const phrases = EditorState.phrases.of({
      Find: text.find,
      next: text.next,
      previous: text.previous,
      all: text.all,
      "match case": text.matchCase,
      regexp: text.regexp,
      "by word": text.byWord,
      close: text.close,
      Replace: text.replaceWith,
      replace: text.replace,
      "replace all": text.replaceAll,
    });

    // The styles of CodeMirror are made by script. The policy of the window allows them only with the nonce that
    // Tauri puts on the style tag of index.html; without a nonce (tests, a page not served by Tauri) nothing is added.
    const styleNonce = document.querySelector<HTMLStyleElement>("style[nonce]")?.nonce;
    const nonce = styleNonce ? EditorView.cspNonce.of(styleNonce) : [];

    // The names of the script API. One source for every editor: the autocompletion tells its sources apart by
    // identity.
    const scriptData = [{ autocomplete: scriptCompletions }];
    const scriptSupport = EditorState.languageData.of(() => scriptData);

    return {
      EditorView,
      EditorState,
      Compartment,
      baseSetup,
      indentation,
      tabIndentation,
      json,
      theme,
      highlight,
      phrases,
      nonce,
      variableSupport,
      scriptSupport,
      openSearchPanel,
      contentReplaced,
      typingGlow,
    };
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import type { EditorView } from "@codemirror/view";
  import type { Compartment, Extension } from "@codemirror/state";
  import { detectIndentUnit } from "../lib/codemirror/indent";
  import { motion } from "../lib/motion.svelte";
  import { useVariableScope } from "../lib/variableScope.svelte";

  interface Props {
    /** Text of the editor. A change made by the user is reported with `onchange`, not written here. */
    value: string;
    /** What the text is highlighted as. */
    language?: "json" | "javascript" | "text";
    /** The text can be selected and copied but not changed. */
    readonly?: boolean;
    /** Accessible name of the text field. */
    label?: string;
    /**
     * The `{{variables}}` of the text are coloured by their scopes, told about under the pointer and offered while
     * one is typed. Needs the variables of an editor around it (`provideVariableScope`).
     */
    variables?: boolean;
    /** What the autocompletion offers besides the variables: `"script"` — the names of the script API (`pm.*`). */
    completions?: "script";
    /** Called with the whole text after every change made by the user. */
    onchange?: (value: string) => void;
  }

  let { value, language = "text", readonly = false, label, variables = false, completions, onchange }: Props = $props();

  /** The variables of the editor around this one. */
  const scope = useVariableScope();

  let host: HTMLDivElement;
  /** The editor once CodeMirror is loaded, with what is needed to reconfigure it. */
  let editor = $state.raw<{
    view: EditorView;
    languageSlot: Compartment;
    readonlySlot: Compartment;
    variablesSlot: Compartment;
    completionsSlot: Compartment;
    indentSlot: Compartment;
    glowSlot: Compartment;
    cm: Awaited<ReturnType<typeof load>>;
  }>();
  /** Text of the document in the editor: tells a change made by the user from one that came with `value`. */
  let shown = "";
  /**
   * One level of indentation of the text, found in the text when it is shown and every time it comes from
   * outside: Enter, Tab and the indent guides follow the text, whatever it is indented with.
   */
  let indentUnit = detectIndentUnit("");
  /** The search panel was asked for before CodeMirror was loaded; it opens with the editor. */
  let searchAsked = false;

  /** Opens the search panel of the editor and puts the focus into its field. */
  export function openSearch(): void {
    if (editor) editor.cm.openSearchPanel(editor.view);
    else searchAsked = true;
  }
  /** The JavaScript language once it is loaded; until then JavaScript is shown without highlighting. */
  let javascript = $state.raw<typeof import("@codemirror/lang-javascript")>();

  /**
   * The extension that highlights the text as `language`. JavaScript is the bare language, not `javascript()`:
   * that one also brings completion sources and snippets, which the script editor does not offer.
   */
  function highlighting(cm: Awaited<ReturnType<typeof load>>) {
    if (language === "json") return cm.json();
    if (language === "javascript" && javascript) return javascript.javascriptLanguage;
    return [];
  }

  /**
   * Whether the text can be changed. Tab indents only where it can: in a text that is read only (a response)
   * Tab moves the focus on, as in the rest of the window.
   */
  function editing(cm: Awaited<ReturnType<typeof load>>): Extension {
    return [cm.EditorState.readOnly.of(readonly), readonly ? [] : cm.tabIndentation];
  }

  /** The indentation unit of the text; the indent guides are for the languages with blocks, not for plain text. */
  function indentation(cm: Awaited<ReturnType<typeof load>>): Extension {
    return cm.indentation(indentUnit, language !== "text");
  }

  /**
   * The extension for the `{{variables}}`, with the variables as they are now. Nothing until they are read:
   * as in a one-line field, a name is then neither of a scope nor "not found".
   */
  function variableSupport(cm: Awaited<ReturnType<typeof load>>): Extension {
    if (!variables || !scope?.loaded) return [];
    return cm.variableSupport(scope.variables);
  }

  /** The extension for the names the autocompletion offers by `completions`. */
  function completionSupport(cm: Awaited<ReturnType<typeof load>>): Extension {
    return completions === "script" ? cm.scriptSupport : [];
  }

  onMount(() => {
    let unmounted = false;
    let view: EditorView | undefined;

    void (loading ??= load()).then((cm) => {
      if (unmounted) return;
      const languageSlot = new cm.Compartment();
      const readonlySlot = new cm.Compartment();
      const variablesSlot = new cm.Compartment();
      const completionsSlot = new cm.Compartment();
      const indentSlot = new cm.Compartment();
      const glowSlot = new cm.Compartment();
      shown = value;
      indentUnit = detectIndentUnit(shown);
      view = new cm.EditorView({
        parent: host,
        state: cm.EditorState.create({
          doc: shown,
          extensions: [
            cm.baseSetup,
            cm.theme,
            cm.highlight,
            cm.phrases,
            cm.nonce,
            languageSlot.of(highlighting(cm)),
            readonlySlot.of(editing(cm)),
            variablesSlot.of(variableSupport(cm)),
            completionsSlot.of(completionSupport(cm)),
            indentSlot.of(indentation(cm)),
            glowSlot.of(cm.typingGlow(motion.rich)),
            cm.EditorView.contentAttributes.of(label ? { "aria-label": label } : {}),
            cm.EditorView.updateListener.of((update) => {
              if (!update.docChanged) return;
              const text = update.state.doc.toString();
              if (text === shown) return;
              shown = text;
              onchange?.(text);
            }),
          ],
        }),
      });
      editor = { view, languageSlot, readonlySlot, variablesSlot, completionsSlot, indentSlot, glowSlot, cm };
      if (searchAsked) cm.openSearchPanel(view);
      searchAsked = false;
    });

    return () => {
      unmounted = true;
      view?.destroy();
    };
  });

  // A text that came from outside (another content, a revert, Beautify) replaces the document; its indentation
  // unit is found anew. At the rich motion level the new text flashes softly once.
  $effect(() => {
    const text = value;
    if (!editor || text === shown) return;
    const { view, indentSlot, cm } = editor;
    shown = text;
    indentUnit = detectIndentUnit(text);
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: text },
      effects: [indentSlot.reconfigure(indentation(cm)), cm.contentReplaced.of(null)],
    });
  });

  // JavaScript is fetched when an editor first needs it; the effect below then switches the highlighting on.
  $effect(() => {
    if (language !== "javascript" || javascript) return;
    let dropped = false;
    void (loadingJavascript ??= import("@codemirror/lang-javascript")).then((module) => {
      if (!dropped) javascript = module;
    });
    return () => {
      dropped = true;
    };
  });

  $effect(() => {
    if (!editor) return;
    const { view, languageSlot, readonlySlot, completionsSlot, indentSlot, cm } = editor;
    view.dispatch({
      effects: [
        languageSlot.reconfigure(highlighting(cm)),
        readonlySlot.reconfigure(editing(cm)),
        completionsSlot.reconfigure(completionSupport(cm)),
        indentSlot.reconfigure(indentation(cm)),
      ],
    });
  });

  // The variables were read again or switched: the tokens are marked anew, the editor and its text stay.
  $effect(() => {
    if (!editor) return;
    const { view, variablesSlot, cm } = editor;
    view.dispatch({ effects: variablesSlot.reconfigure(variableSupport(cm)) });
  });

  // The glow of typed, pasted and copied text exists at the rich motion level only; saving another level in the
  // settings switches it in every open editor.
  $effect(() => {
    if (!editor) return;
    const { view, glowSlot, cm } = editor;
    view.dispatch({ effects: glowSlot.reconfigure(cm.typingGlow(motion.rich)) });
  });
</script>

<!-- Wrapper of CodeMirror 6. The library is a separate chunk fetched by the first editor on the screen. -->
<div bind:this={host} class="code-editor"></div>

<style>
  .code-editor {
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }
</style>
