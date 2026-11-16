<script lang="ts">
  // Stands in for `CodeEditor.svelte` in component tests: CodeMirror needs a layout that jsdom does not have.
  // Not imported by the application, so it is not part of its build.
  interface Props {
    value: string;
    language?: "json" | "javascript" | "text";
    readonly?: boolean;
    label?: string;
    onchange?: (value: string) => void;
  }

  let { value, language = "text", readonly = false, label, onchange }: Props = $props();
</script>

<textarea
  aria-label={label}
  data-language={language}
  {readonly}
  {value}
  oninput={(event) => onchange?.(event.currentTarget.value)}
></textarea>
