<script lang="ts" module>
  /** Everything of CodeMirror the editor uses. Loaded once, when the first editor is shown. */
  let loading: ReturnType<typeof load> | undefined;
  /** The JavaScript language: a chunk of its own, loaded once, by the first editor that shows JavaScript. */
  let loadingJavascript: Promise<typeof import("@codemirror/lang-javascript")> | undefined;

  async function load() {
    const [{ EditorView, basicSetup }, { Compartment, EditorState }, { json }, { HighlightStyle, syntaxHighlighting }, { tags }] =
      await Promise.all([
        import("codemirror"),
        import("@codemirror/state"),
        import("@codemirror/lang-json"),
        import("@codemirror/language"),
        import("@lezer/highlight"),
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
        ".cm-foldPlaceholder": { backgroundColor: "var(--surface-2)", color: "var(--text-muted)", border: "none" },
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

    return { EditorView, EditorState, Compartment, basicSetup, json, theme, highlight };
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import type { EditorView } from "codemirror";
  import type { Compartment } from "@codemirror/state";

  interface Props {
    /** Text of the editor. A change made by the user is reported with `onchange`, not written here. */
    value: string;
    /** What the text is highlighted as. */
    language?: "json" | "javascript" | "text";
    /** The text can be selected and copied but not changed. */
    readonly?: boolean;
    /** Accessible name of the text field. */
    label?: string;
    /** Called with the whole text after every change made by the user. */
    onchange?: (value: string) => void;
  }

  let { value, language = "text", readonly = false, label, onchange }: Props = $props();

  let host: HTMLDivElement;
  /** The editor once CodeMirror is loaded, with what is needed to reconfigure it. */
  let editor = $state.raw<{
    view: EditorView;
    languageSlot: Compartment;
    readonlySlot: Compartment;
    cm: Awaited<ReturnType<typeof load>>;
  }>();
  /** Text of the document in the editor: tells a change made by the user from one that came with `value`. */
  let shown = "";
  /** The JavaScript language once it is loaded; until then JavaScript is shown without highlighting. */
  let javascript = $state.raw<typeof import("@codemirror/lang-javascript")>();

  /**
   * The extension that highlights the text as `language`. JavaScript is the bare language, not `javascript()`:
   * that one also brings completion sources and snippets, and a snippet takes Tab away from leaving the editor.
   */
  function highlighting(cm: Awaited<ReturnType<typeof load>>) {
    if (language === "json") return cm.json();
    if (language === "javascript" && javascript) return javascript.javascriptLanguage;
    return [];
  }

  onMount(() => {
    let unmounted = false;
    let view: EditorView | undefined;

    void (loading ??= load()).then((cm) => {
      if (unmounted) return;
      const languageSlot = new cm.Compartment();
      const readonlySlot = new cm.Compartment();
      shown = value;
      view = new cm.EditorView({
        parent: host,
        state: cm.EditorState.create({
          doc: shown,
          extensions: [
            cm.basicSetup,
            cm.theme,
            cm.highlight,
            languageSlot.of(highlighting(cm)),
            readonlySlot.of(cm.EditorState.readOnly.of(readonly)),
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
      editor = { view, languageSlot, readonlySlot, cm };
    });

    return () => {
      unmounted = true;
      view?.destroy();
    };
  });

  // A text that came from outside (another content, a revert) replaces the document.
  $effect(() => {
    const text = value;
    if (!editor || text === shown) return;
    shown = text;
    editor.view.dispatch({ changes: { from: 0, to: editor.view.state.doc.length, insert: text } });
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
    const { view, languageSlot, readonlySlot, cm } = editor;
    view.dispatch({
      effects: [
        languageSlot.reconfigure(highlighting(cm)),
        readonlySlot.reconfigure(cm.EditorState.readOnly.of(readonly)),
      ],
    });
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
