<script lang="ts">
  // Measuring panel of the window probe (stage 0 spike). Throwaway code, not the product UI.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { EditorView, basicSetup } from "codemirror";
  import { EditorState } from "@codemirror/state";
  import { json } from "@codemirror/lang-json";

  type Format = "pretty" | "minified";
  type Transport = "string" | "bytes";
  type Run = {
    n: number;
    cell: string;
    firstCall: boolean;
    sizeMb: number;
    transferMs: number;
    decodeMs: number | null;
    editorMs: number;
  };

  const SIZES = [1, 5, 10, 20];
  const FORMATS: Format[] = ["pretty", "minified"];
  const TRANSPORTS: Transport[] = ["string", "bytes"];
  const EXPECTED_DB_VALUE = "Привет, мир — проверка";

  let editorHost: HTMLDivElement;
  let view: EditorView;
  let busy = $state(false);
  let status = $state("");
  let runs = $state<Run[]>([]);
  let lastKeyMs = $state<number | null>(null);
  let maxKeyMs = $state<number | null>(null);
  let dbResult = $state<{ value: string; path: string; ok: boolean } | null>(null);
  // Cells already requested once: their first call also includes body generation in Rust.
  const requested = new Set<string>();

  const extensions = [basicSetup, json(), EditorState.readOnly.of(true)];

  // Resolves after the next frame has been painted.
  function painted(): Promise<void> {
    return new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
  }

  async function showDocument(text: string): Promise<number> {
    const start = performance.now();
    view.setState(EditorState.create({ doc: text, extensions }));
    await painted();
    return performance.now() - start;
  }

  async function measure(mb: number, format: Format, transport: Transport) {
    busy = true;
    const cell = `${mb} МБ · ${format} · ${transport === "string" ? "строка" : "байты"}`;
    status = `Идёт: ${cell}…`;
    await painted(); // let the status repaint before the measured work
    try {
      const firstCall = !requested.has(`${mb}/${format}`);
      requested.add(`${mb}/${format}`);

      const minified = format === "minified";
      const t0 = performance.now();
      let text: string;
      let decodeMs: number | null = null;
      let transferMs: number;
      if (transport === "string") {
        text = await invoke<string>("gen_json", { mb, minified });
        transferMs = performance.now() - t0;
      } else {
        const buffer = await invoke<ArrayBuffer>("gen_bytes", { mb, minified });
        transferMs = performance.now() - t0;
        const d0 = performance.now();
        text = new TextDecoder().decode(buffer);
        decodeMs = performance.now() - d0;
      }
      const editorMs = await showDocument(text);
      runs = [
        {
          n: runs.length + 1,
          cell,
          firstCall,
          sizeMb: text.length / (1024 * 1024),
          transferMs,
          decodeMs,
          editorMs,
        },
        ...runs,
      ];
      status = `Готово: ${cell}. Теперь проверьте ввод в поле ниже.`;
      lastKeyMs = null;
      maxKeyMs = null;
    } catch (e) {
      status = `Ошибка: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function clearEditor() {
    await showDocument("");
    status = "Редактор очищен.";
  }

  async function checkDb() {
    try {
      const [value, path] = (await invoke<string>("db_roundtrip")).split("\n");
      dbResult = { value, path, ok: value === EXPECTED_DB_VALUE };
    } catch (e) {
      dbResult = { value: `Ошибка: ${e}`, path: "", ok: false };
    }
  }

  // Delay from the key press to the frame that shows it.
  function onKey(event: KeyboardEvent) {
    const pressedAt = event.timeStamp;
    requestAnimationFrame(() => {
      const delay = performance.now() - pressedAt;
      lastKeyMs = delay;
      maxKeyMs = Math.max(maxKeyMs ?? 0, delay);
    });
  }

  const ms = (v: number | null) => (v === null ? "—" : v.toFixed(0));

  onMount(() => {
    view = new EditorView({ parent: editorHost, state: EditorState.create({ doc: "", extensions }) });
    // After the first paint of the window: report readiness for measure-start.ps1.
    painted().then(() => invoke("mark_ready"));
  });
</script>

<main>
  <h1>RequestMaster Spike — панель замеров</h1>

  <section>
    <h2>1. Большой ответ</h2>
    <p class="hint">
      Нажмите кнопку — тело придёт из ядра и откроется в редакторе. Первое нажатие для пары «размер ·
      формат» включает генерацию данных в Rust (отмечено «*»), берите числа со второго нажатия.
    </p>
    <table class="matrix">
      <thead>
        <tr>
          <th>Размер</th>
          {#each FORMATS as format}
            {#each TRANSPORTS as transport}
              <th>{format} · {transport === "string" ? "строка" : "байты"}</th>
            {/each}
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each SIZES as mb}
          <tr>
            <td>{mb} МБ</td>
            {#each FORMATS as format}
              {#each TRANSPORTS as transport}
                <td><button disabled={busy} onclick={() => measure(mb, format, transport)}>Запустить</button></td>
              {/each}
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
    <p>
      <button disabled={busy} onclick={clearEditor}>Очистить редактор</button>
      <span class="status">{status}</span>
    </p>
  </section>

  <section>
    <h2>2. Отзывчивость ввода</h2>
    <p class="hint">
      Пока в редакторе открыт большой документ, напечатайте здесь несколько слов. Задержка — от нажатия
      клавиши до кадра, который её показал. Сбрасывается при каждом новом замере.
    </p>
    <input type="text" placeholder="печатайте здесь" onkeydown={onKey} />
    <span>последняя: <b>{ms(lastKeyMs)} мс</b>, максимум: <b>{ms(maxKeyMs)} мс</b></span>
  </section>

  <section>
    <h2>3. Результаты (новые сверху)</h2>
    <table class="runs">
      <thead>
        <tr>
          <th>#</th>
          <th>Ячейка</th>
          <th>Факт. размер, МБ</th>
          <th>Передача, мс</th>
          <th>Декодирование байтов, мс</th>
          <th>Готовность редактора, мс</th>
        </tr>
      </thead>
      <tbody>
        {#each runs as run (run.n)}
          <tr>
            <td>{run.n}</td>
            <td>{run.cell}{run.firstCall ? " *" : ""}</td>
            <td>{run.sizeMb.toFixed(2)}</td>
            <td>{ms(run.transferMs)}</td>
            <td>{ms(run.decodeMs)}</td>
            <td>{ms(run.editorMs)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>

  <section>
    <h2>4. SQLite</h2>
    <button onclick={checkDb}>Проверить db_roundtrip</button>
    {#if dbResult}
      <p>
        Прочитано: «{dbResult.value}» — {dbResult.ok ? "совпадает" : "НЕ совпадает"} с «{EXPECTED_DB_VALUE}»<br />
        Файл базы: <code>{dbResult.path}</code>
      </p>
    {/if}
  </section>

  <section>
    <h2>Редактор</h2>
    <div class="editor" bind:this={editorHost}></div>
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: "Segoe UI", sans-serif;
    font-size: 14px;
    background: #fff;
    color: #222;
  }
  main {
    padding: 12px 16px;
  }
  h1 {
    font-size: 18px;
    margin: 0 0 8px;
  }
  h2 {
    font-size: 15px;
    margin: 12px 0 4px;
  }
  .hint {
    color: #555;
    margin: 0 0 6px;
  }
  table {
    border-collapse: collapse;
  }
  th,
  td {
    border: 1px solid #ccc;
    padding: 3px 8px;
    text-align: left;
  }
  .status {
    margin-left: 8px;
  }
  input {
    width: 320px;
    margin-right: 8px;
  }
  .editor {
    height: 360px;
    border: 1px solid #ccc;
  }
  .editor :global(.cm-editor) {
    height: 100%;
  }
</style>
