# RequestMaster, этап 1, часть 3 (план 1) «Новый вид, работа с коллекциями, история» — план реализации

**Goal:** Программой можно пользоваться каждый день без Postman под рукой: создавать и удалять запросы и папки,
править окружения, выгружать коллекцию обратно в Postman, смотреть историю отправок — в оформлении «А · Графит»,
которое выбрал владелец.

**Architecture:** Как в части 2: логика — в `AppCore` (`src-tauri`, Rust, без типов Tauri, под тестами), команды
Tauri только вызывают его; интерфейс `ui/` (Svelte 5 + TypeScript) не содержит логики Postman, переменных и HTTP.
Ядро `crates/rqm-*` меняется только там, где задача это прямо называет.

**Tech Stack:** то же, что в части 2, плюс `tauri-plugin-dialog` (системное окно «Сохранить как» для экспорта;
`=2.8.4` — последняя 2.x, требует `tauri ^2.12`; перед закреплением сверить `cargo info tauri-plugin-dialog@2.8.4`, версию 3.0-alpha не брать) — новая зависимость **[моё решение]**.

**Spec:** `docs/design.md` (разделы 6, 9, 15 пп. 6–7). Окно части 2 и его
контракт команд: `docs/plans/stage1b-window.md`, итоги —
`stage1b-results.md`. Макет оформления: артборд «А · Графит»; его исходник лежит локально не в репозитории, поэтому
все нужные значения выписаны в Task 2.

## Что входит и что остаётся на план 2 этой части

Входит: оформление «А · Графит» (значки, кнопки, поля, вкладки, меню, пустые состояния); создание, переименование,
дублирование и удаление коллекций, папок и запросов; редактор окружений и глобальных переменных; экспорт коллекции
и окружения в файл Postman; копирование ответа; история отправок.

Остаётся на план 2 **[моё решение]**: перетаскивание в дереве; редактор папки; вкладки Scripts и Settings (скрипты
всё равно начнут выполняться только на этапе 2); подсветка и автодополнение `{{переменных}}`; запоминание вкладок,
черновиков и размеров панелей между запусками; настройки приложения; «Сохранить в файл» для ответа больше 20 МБ;
виртуализация дерева; замеры раздела 10 спеки; строгая CSP.

## Global Constraints

- Ветка: `feature/stage1-part3`, создаётся от `feature/stage1b-window`. В `main`/`master` не коммитить. Push — никогда.
- Сообщения коммитов на русском.
- Приложение, собранный exe и dev-серверы не запускать; `tauri build` — только в Task 7. Разрешено: `cargo
  build/test/clippy`, а внутри `ui/` — `npm install`, `npm run check`, `npm run test`, `npm run build`.
- Сеть: crates.io и npm. Тесты ходят только на `127.0.0.1`; в тестах отправки задавать `NO_PROXY=127.0.0.1`.
- Версии зависимостей закреплять точно; у npm-пакетов поле `engines` должно допускать установленный Node 22.12.
- Схема БД меняется ровно на DDL из Task 1 (таблица `history`), согласованный владельцем вместе с этим планом.
  Любая другая таблица, колонка или индекс — остановиться и спросить.
- В TypeScript нет разбора и сборки Postman-JSON, подстановки переменных, сборки HTTP-запроса, форматирования JSON.
- Тексты интерфейса — в `ui/src/lib/strings.ts` под английскими ключами: сообщения и подписи на русском, термины
  HTTP и Postman на английском. В компонентах русского текста нет. Все цвета — через переменные `tokens.css`.
- Каждая команда окна должна быть в четырёх местах: `COMMANDS` (`src-tauri/src/lib.rs`), `build.rs`,
  `generate_handler!` (`main.rs`), `capabilities/default.json` (`allow-<имя-через-дефис>`). Тест
  `commands_are_listed_everywhere` обновляется в каждой задаче, которая добавляет команды.
- Команды асинхронные (`async fn`, `Result<_, String>`), ключи аргументов и ответов — `snake_case`; замок
  хранилища через `.await` не удерживается. Верхний `postman_extra` узла в окно не передаётся (правило части 2).
- Данные владельца: `spike/samples/` в git не попадает; значения из образцов не печатать и не копировать.
- Только то, что в задаче. Доступность: всё с клавиатуры, подписи у значков-кнопок (`aria-label`), фокус виден.
- Rust-задача заканчивается зелёными `cargo test --workspace` и `cargo clippy --workspace --all-targets -- -D
  warnings`; UI-задача — зелёными `npm run check`, `npm run test`, `npm run build` в `ui/`.

## Review Focus

1. **Удаление узла с открытыми вкладками** — вкладки удалённого запроса (и всех запросов удалённой папки или
   коллекции) закрываются без вопроса о сохранении; идущая отправка отменяется (тест в Task 3).
2. **Переименование при открытой вкладке с черновиком** — имя на вкладке и в дереве меняется, черновик и точка
   «не сохранено» остаются (тест в Task 3).
3. **Удаление активного окружения** — выбор сбрасывается на «Без окружения», следующая отправка идёт без него
   (тесты в Task 1 и Task 4).
4. **Экспорт отменён в системном окне** — не ошибка и не уведомление об успехе (тесты в Task 1 и Task 5).
5. **История не раздувает базу и не теряет ответ** — тело больше 1 МиБ обрезается с пометкой, записей не больше
   500, сбой записи истории не отнимает ответ у пользователя (тесты в Task 1).

---

### Task 1: Оболочка — команды дерева, окружений, экспорта и истории

**Files:**
- Modify: `crates/rqm-runner/src/send.rs`, `crates/rqm-runner/src/lib.rs` (поле `request`), `crates/rqm-runner/tests/send.rs`
- Modify: `crates/rqm-store/src/lib.rs`, `crates/rqm-store/tests/store.rs` (`schema.sql` не меняется)
- Create: `crates/rqm-store/src/schema_v2.sql`, `crates/rqm-store/src/history.rs`
- Modify: `Cargo.toml` (корень: `tauri-plugin-dialog`), `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`,
  `src-tauri/src/core.rs`, `src-tauri/src/dto.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`,
  `src-tauri/src/send.rs`, `src-tauri/build.rs`, `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/history.rs`, `src-tauri/src/export.rs`
- Test: `src-tauri/tests/core_tree.rs`, `src-tauri/tests/core_history.rs`; обновить `src-tauri/tests/core_data.rs`
  и `src-tauri/tests/core_send.rs` (новое поле `CoreConfig`)

**DDL — отдельный файл `crates/rqm-store/src/schema_v2.sql`; согласуется владельцем вместе с этим планом, менять нельзя:**

```sql
CREATE TABLE history (
    -- История отправок: одна строка на каждое нажатие Send, включая неудачные и отменённые.
    -- Читают: раздел «История» в окне (список и просмотр записи).

    -- Порядковый номер записи; назначает SQLite. По нему список сортируется «новые сверху» и удаляются старые.
    id      INTEGER PRIMARY KEY,
    -- Момент начала отправки, UTC, вид 2026-11-13T10:42:07.123Z; из системных часов. Показывается в списке.
    sent_at TEXT NOT NULL,
    -- Короткие сведения для списка, JSON: id запроса в дереве, его имя на момент отправки, метод и адрес как
    -- ушли в сеть, статус ответа или вид ошибки, полное время в мс, размер ответа. Пишет оболочка после отправки.
    summary TEXT NOT NULL,
    -- Подробности для просмотра записи, JSON: заголовки и тело запроса как ушли, заголовки и тело ответа,
    -- текст ошибки, предупреждения. Тела длиннее 1 МиБ обрезаны с пометкой. Пишет оболочка после отправки.
    detail  TEXT NOT NULL
) STRICT;
```

Версия схемы становится `2`. `schema.sql` (три таблицы версии 1) не меняется. Открытие базы, каждый случай в
одной транзакции: пустая база — `schema.sql`, затем `schema_v2.sql`, `PRAGMA user_version = 2`; версия 1 —
только `schema_v2.sql` и `PRAGMA user_version = 2`, существующие таблицы и данные не трогаются; версия 2 —
работать; версия 0 с уже существующими таблицами — прежняя ошибка; больше 2 — `NewerSchema`. Индексов нет.
Две колонки JSON вместо одной — чтобы список из 500 записей не читал тела; отдельной колонки под каждое поле
списка нет: `summary` разбирает оболочка. Doc-комментарий `rqm-store/src/lib.rs` («three tables») обновить.

**Interfaces:**
- Consumes: `rqm-store` (`create_node`, `rename_node`, `delete_node`, `duplicate_node`, `subtree`,
  `environments`, `globals`, `save_environment`, `delete_environment`), `rqm-postman`
  (`export_collection`, `export_environment`), `rqm-runner`.
- Produces — изменение ядра:

```rust
// rqm-runner
pub struct SendOutcome { pub result: …, pub warnings: …,
    /// What was prepared for sending, for the history. `None` only when `prepare` failed.
    pub request: Option<PreparedSummary> }
/// Filled in `send()` from `Prepared.http` before the engine is called (the engine takes the request by value).
pub struct PreparedSummary { pub method: String, pub url: String,
    /// The request's own headers and the auth header — without the automatic ones.
    pub headers: Vec<(String, String)>,
    /// Text form of the body: bytes as lossy UTF-8; for multipart one line per part (`name: value` or
    /// `name: <file path>`); for a file body `<file: path>`; `None` when no body is sent (also when it was pruned).
    pub body: Option<String> }

// rqm-store
pub struct HistoryRow { pub id: i64, pub sent_at: String, pub summary: String }
impl Store {
    /// Appends an entry and deletes the oldest ones beyond `keep` (one transaction); returns the new id.
    pub fn history_add(&mut self, sent_at: &str, summary: &str, detail: &str, keep: usize) -> Result<i64, StoreError>;
    /// Newest first; `before_id` = only entries older than that id.
    pub fn history_list(&self, limit: usize, before_id: Option<i64>) -> Result<Vec<HistoryRow>, StoreError>;
    pub fn history_detail(&self, id: i64) -> Result<String, StoreError>;      // NotFound
    pub fn history_clear(&mut self) -> Result<(), StoreError>;
}
```

- Produces — в `src-tauri`: `pub struct CoreConfig { pub db_path, pub bodies_dir, pub history_keep: Option<usize> }`
  (`None` = 500); во все существующие литералы (`main.rs`, `core_data.rs`, `core_send.rs`) дописать
  `history_keep: None`. Новая зависимость оболочки от своего же ядра: `rqm-vars` (время записи истории).
- Produces — новые команды окна (14; всего станет 27):

```text
create_node(parent_id: string|null, kind: "collection"|"folder"|"request", name) -> string   // id нового узла
rename_node(id, name) -> null
duplicate_node(id) -> string
delete_node(id) -> null
get_environment(id) -> EnvironmentDto         EnvironmentDto = { id, name, variables: VariableDto[] }
                                              VariableDto = { key, value, secret, enabled, postman_extra }
    // отдельная структура в dto.rs: все пять ключей есть в JSON всегда (postman_extra — объект, возможно пустой);
    // при приёме отсутствующие secret / enabled / postman_extra означают false / true / {}
save_environment(id, name, variables) -> null // id "globals" — глобальные переменные (имя не меняется)
create_environment(name) -> string
delete_environment(id) -> null                // "globals" удалить нельзя
export_collection(id) -> string|null          // путь сохранённого файла; null — пользователь закрыл окно выбора
export_environment(id) -> string|null
history_list(limit: number, before_id: number|null) -> HistoryItem[]
    HistoryItem = { id, sent_at, node_id: string|null, name, method, url, status: number|null,
                    error_kind: string|null, total_ms: number|null, size: number|null }
history_get(id) -> HistoryEntry
    HistoryEntry = HistoryItem & { request_headers: [string,string][], request_body: string|null,
                    request_body_truncated: boolean, response_headers: [string,string][],
                    response_body: string|null, response_body_truncated: boolean, response_body_binary: boolean,
                    error_message: string|null, warnings: Warning[], node_exists: boolean }
history_clear() -> null
copy_response_body(body_id, pretty: boolean) -> string   // текст для буфера обмена; не-UTF-8 — с заменой символов
```

**Правила:**
- `create_node`: имя обрезается по краям; пустое → ошибка `name is empty`. Содержимое нового узла: коллекция и
  папка — пустые; запрос — `GET`, пустой URL, без тела, `auth` не задан (наследуется). Узел встаёт последним.
  Правила размещения — из `rqm-store` (`InvalidMove` → понятное сообщение).
- `rename_node`: то же правило имени. `delete_node`: удаляет с потомками. `duplicate_node` — как в `rqm-store`.
- `get_environment("globals")` отдаёт глобальные переменные (пустой набор, если их ещё нет).
  `save_environment`: имя обрезается; пустое → ошибка (для `globals` имя игнорируется); `postman_extra`
  окружения берётся из сохранённого, окно присылает только переменные. `get_environment` и `save_environment`
  с id, которого нет среди окружений и который не `globals`, — ошибка `environment not found` (хранилище само
  создало бы новое окружение — этого допускать нельзя). `delete_environment`: если удалено активное окружение —
  ключ `active_environment` сбрасывается.
- Экспорт: текст — `rqm_postman::export_collection(subtree)` / `export_environment`, JSON с отступом 2 пробела
  (`serde_json::to_string_pretty` допустим: это наш вывод, а не данные пользователя; порядок ключей сохраняет
  `preserve_order`). Имя по умолчанию: `<имя>.postman_collection.json` / `<имя>.postman_environment.json` /
  `globals.postman_globals.json`; символы `\ / : * ? " < > |` в имени заменяются на `_`. Сборка текста и имени —
  в `AppCore` (`export_collection_text(id) -> (file_name, text)`, под тестами); системное окно «Сохранить как» и
  запись файла — в команде: в `main.rs` зарегистрировать `.plugin(tauri_plugin_dialog::init())` (без этого
  первый же экспорт уронит программу); команды `export_*` принимают `app: tauri::AppHandle` и вызывают
  `app.dialog().file().set_file_name(&name).add_filter("JSON", &["json"]).blocking_save_file()` (из `async`-команды
  это допустимо); `None` → `Ok(None)`; путь — `FilePath::into_path()`, запись — `std::fs::write`. В
  `capabilities/default.json` разрешений плагина не добавлять: Rust-стороне они не нужны, а тест списка
  разрешений сравнивает на точное равенство. Не коллекция → ошибка.
- История пишется в `send_request` сразу после `rqm_runner::send`, до сохранения cookies и до сборки ответа для
  окна (файл тела ещё на месте), при любом исходе отправки; ошибка `history_add` отбрасывается и результат
  команды не меняет. `sent_at` — `rqm_vars::system_dynamic("isoTimestamp")`, взятое до вызова отправки.
- Что в записи. Есть ответ: метод, адрес и заголовки — из `HttpResponse.sent`. Ответа нет, но
  `SendOutcome.request` есть (ошибка сети, таймаут, отмена): метод, адрес и заголовки — из него, то есть
  переменные уже подставлены. `prepare` упал: метод и адрес из запроса как набраны, заголовков нет, тело `null`.
  Тело запроса — `request.body`. `status`, `total_ms` (`timings.total` в мс) и `size` (`decoded_size`) — из
  ответа, без ответа — `null`. Отменённая отправка — `error_kind = "cancelled"`. Хранится не больше
  `history_keep` записей.
- Тело в истории (и запроса, и ответа): прочитать не больше 1 048 576 байт; `std::str::from_utf8` дал ошибку с
  `error_len() == Some(_)` → для ответа `response_body_binary = true` и тело `null` (для запроса — с заменой
  символов); ошибка с `error_len() == None` (символ разрезан срезом) → взять `valid_up_to()` байт;
  `*_truncated = true`, когда исходный размер больше 1 МиБ; пустое тело — пустая строка, не «не текстовое».
- Заголовки запроса хранятся как ушли, включая `Authorization` — как и остальные данные, в открытом виде в
  локальной базе **[моё решение]**, вынесено владельцу.
- `history_get`: `node_exists` — есть ли узел `node_id` в дереве сейчас. `history_list`: `limit` от 1 до 200.
- `copy_response_body` — по правилам `rqm_runner::body_view`; обрезанное тело отдаётся как показано.

- [ ] **Step 1: Ядро.** Падающие тесты: в `rqm-runner/tests/send.rs` — `request` при успешной отправке (метод,
  адрес и тело после подстановки переменных, заголовки запроса и auth без автоматических), для multipart
  (строка на часть), `body = None` для запроса без тела и для обрезанного тела `GET`, `request` заполнен и при
  ошибке соединения, `None` при ошибке подготовки; в
  `rqm-store/tests/store.rs` — новая база имеет версию 2 и четыре таблицы; база версии 1 (создать вручную
  старым DDL из git-истории `schema.sql` — три таблицы и `user_version = 1`, с одной коллекцией) открывается,
  получает `history`, версию 2 и сохраняет коллекцию; `history_add` с `keep = 3` после пяти добавлений оставляет
  три новейшие; `history_list` — новые сверху, `before_id` и `limit` работают; `history_detail` неизвестного id →
  `NotFound`; `history_clear`. Тест `schema_has_exactly_three_tables_and_version_1` переписать под четыре
  таблицы и версию 2 (проверка комментариев — и для `history`); в `reopen_keeps_data_and_newer_schema_is_refused`
  заменить 2 на 3 (`PRAGMA user_version = 3`, `NewerSchema(3)`). Затем реализация.
- [ ] **Step 2: Оболочка.** Падающие тесты `core_tree.rs`: создать коллекцию, папку и запрос — дерево и содержимое
  по правилам; пустое имя → ошибка; запрос в корень → ошибка; переименование; дублирование папки — копия рядом
  с новыми id; удаление папки убирает потомков; окружения — создать, сохранить переменные (включая секретную
  и выключенную), прочитать, `postman_extra` импортированного окружения переживает сохранение из окна,
  удаление активного окружения сбрасывает выбор, `globals` нельзя удалить, но можно сохранить и прочитать;
  JSON ответа `get_environment` содержит все пять ключей у обычной переменной; неизвестный id в
  `get_environment` и `save_environment` — ошибка, нового окружения не появилось;
  `export_collection_text` импортированной коллекции — `semantic_diff` с исходником пуст (dev-зависимость
  `rqm-postman` уже есть), имя файла с заменой запрещённых символов; экспорт не-коллекции → ошибка.
  `core_history.rs` (wiremock): успешная отправка даёт запись с методом, адресом, статусом, телами; ошибка
  соединения — запись с `error_kind = "connect"`, в адресе которой переменная подставлена; отмена —
  `"cancelled"`; пустой URL — запись с адресом как набран и без заголовков; ответ 2 МиБ из ASCII —
  `response_body` ровно 1 МиБ и флаг обрезки; ответ больше 1 МиБ из двухбайтных символов со смещением в один
  байт — запись текстовая, обрезана, не длиннее 1 МиБ; не-UTF-8 ответ — `response_body_binary`; при
  `CoreConfig { db_path: None, bodies_dir, history_keep: Some(3) }` четвёртая отправка вытесняет первую; `history_get`
  после удаления запроса — `node_exists = false`; `history_clear`; `copy_response_body` в обоих видах.
  `commands_are_listed_everywhere` — 27 команд. Затем реализация и команды.
- [ ] **Step 3: `cargo test --workspace` и clippy** — зелёные; записать числа в отчёт.
- [ ] **Step 4: Commit** — «Часть 3: команды дерева, окружений, экспорта и истории».

---

### Task 2: Оформление «А · Графит» — набор компонентов

**Files:**
- Modify: `ui/src/styles/tokens.css`, `ui/src/styles/base.css`, `ui/src/styles/contrast.test.ts`, `ui/src/lib/strings.ts`
- Modify: `ui/src/components/CodeEditor.svelte`, `ui/src/components/Tree.svelte`, `ui/src/lib/api.ts`,
  `ui/src/lib/testing/fakeApi.ts`, `ui/src/components/ResponsePanel.test.ts`
- Create: `ui/src/components/EnvironmentSwitcher.test.ts`, `ui/src/components/ui/Icon.svelte`, `ui/src/components/ui/Button.svelte`,
  `ui/src/components/ui/IconButton.svelte`, `ui/src/components/ui/Field.svelte`,
  `ui/src/components/ui/Segmented.svelte`, `ui/src/components/ui/Menu.svelte`,
  `ui/src/components/ui/EmptyState.svelte`, `ui/src/lib/icons.ts`
- Modify (перевести на новые компоненты, поведение не менять): `ui/src/App.svelte`, `Sidebar.svelte`,
  `TreeRow.svelte`, `TabBar.svelte`, `UrlBar.svelte`, `RequestEditor.svelte`, `BodyEditor.svelte`,
  `AuthEditor.svelte`, `KeyValueTable.svelte`, `CollectionEditor.svelte`, `ResponsePanel.svelte`,
  `ResponseStatus.svelte`, `ResponseBody.svelte`, `EnvironmentSwitcher.svelte`, `Dialog.svelte`, `Toasts.svelte`
- Test: `ui/src/components/ui/ui.test.ts`; существующие тесты остаются зелёными без ослабления

**Значения оформления (из макета «А · Графит»):**

```css
--bg: #14171c; --surface: #1a1e25; --surface-2: #232831; --border: #2b313c;
--text: #e1e5ec; --text-muted: #9aa3b2; --accent: #2fc7c0; --accent-contrast: #07201f;
--method-get: #4cc38a; --method-post: #e5a93b; --method-put: #6aa6f8; --method-patch: #a98bf5;
--method-delete: #f07178; --method-other: #9aa3b2;
--code-key: #7cb7ff; --code-string: #4cc38a; --code-number: #e5a93b; --code-atom: #b59cf7; --code-line: #626b7a;
--control-height: 32px; --control-height-sm: 28px; --row-height: 30px; --tab-height: 40px;
```

  `--code-line` чуть светлее, чем в макете (`#5b6472`), ради контраста номеров строк **[моё решение]**.
  Остальные токены части 2 сохраняются. Шрифты: интерфейс — Inter, если установлен, иначе Segoe UI Variable;
  данные — JetBrains Mono, иначе Cascadia Mono, Consolas (в приложение шрифты не упаковываются).
- `Icon` — встроенные контурные SVG 16 px (`stroke: currentColor`, толщина 1.6, скруглённые концы), имена:
  `chevron-down`, `chevron-right`, `search`, `plus`, `send`, `copy`, `close`, `import`, `export`, `trash`,
  `edit`, `duplicate`, `folder`, `collection`, `environment`, `history`, `more`, `check`. Декоративный значок —
  `aria-hidden`.
- `Button` — высота 32 px (малая 28), граница 1 px `--border`, радиус 4 px; варианты `default`, `primary`
  (заливка `--accent`, текст `--accent-contrast`, жирность 600), `ghost` (без границы, приглушённый текст).
  `IconButton` — квадратная, обязательный `label` → `aria-label`.
- `Field` — поле 32 px с необязательным значком слева; фон `--bg`, граница `--border`.
- `Segmented` — группа кнопок с общей границей для разделов боковой панели (его подключит Task 4):
  `role="tablist"` / `"tab"`, `aria-selected`, стрелки влево/вправо, у варианта может быть `disabled` (стрелки его
  пропускают). Переключатель Pretty / Raw в ответе остаётся как есть (`role="group"`, кнопки с `aria-pressed`,
  неактивный Pretty для не-JSON) — меняются только его стили под общий вид.
- `Menu` — всплывающий список действий у кнопки или у точки щелчка: `role="menu"`, стрелки, Enter, Escape,
  закрытие по щелчку вне; пункт может быть «опасным» (цвет `--danger`). Появление — `opacity` и сдвиг 4 px за
  `--dur-2`; при `prefers-reduced-motion` — без движения.
- `EmptyState` — значок, заголовок, строка пояснения, необязательная кнопка.
- Что меняется на экране: вкладки запросов — высота 40 px, активная с линией акцента сверху и фоном `--bg`;
  разделы редактора и ответа — подчёркивание акцентом, счётчик цветом акцента; в дереве значки-стрелки у папок и
  коллекций, строка 30 px, активная строка `--surface-2`, точка несохранённого справа — декоративная
  (`aria-hidden="true"`, без подписи, класс `dirty`; признак считает `Tree.svelte`: есть открытая вкладка этого
  узла с несохранёнными правками — и передаёт в `TreeRow`), доступное имя строки не меняется; поле фильтра со значком
  поиска; кнопка Send — `primary` со значком; переключатель окружения — кнопка с цветной точкой (зелёная, когда
  окружение выбрано) и стрелкой, по щелчку — `Menu` со всеми окружениями и пунктом «Без окружения» (сейчас это
  системный `<select>`; логика выбора и отката при сбое прежняя); редактор кода (`CodeEditor.svelte`) —
  `--code-key` для имён свойств, `--code-string`, `--code-number`, `--code-atom` для `true`/`false`/`null`,
  полоса номеров строк: фон `var(--bg)`, цвет `var(--code-line)`; вместо надписей
  «Здесь появятся…» — `EmptyState` («Откройте запрос из списка слева» и т.п.); кнопка «Копировать» в строке
  статуса ответа (вызывает `copy_response_body`, пишет в буфер через `navigator.clipboard.writeText`, уведомление
  «Скопировано»; если буфер недоступен — уведомление об ошибке).

- [ ] **Step 1: Падающие тесты `ui.test.ts`:** `Button` — три варианта дают разные классы, `disabled` не вызывает
  обработчик; `IconButton` без `label` — ошибка типов (проверяется `npm run check`), с `label` — `aria-label`;
  `Segmented` — стрелка вправо переносит выбор и вызывает `onchange`; `Menu` — открывается, стрелки двигают
  фокус, Enter вызывает действие и закрывает, Escape и щелчок вне закрывают, опасный пункт имеет свой класс;
  `EmptyState` — заголовок и кнопка; `Icon` — неизвестное имя не компилируется (тип-объединение имён).
  `contrast.test.ts` — новые пары: `--code-key`, `--code-string`, `--code-number`, `--code-atom` на `--bg` не
  ниже 4,5; отдельной проверкой `--code-line` на `--bg` не ниже 3. `EnvironmentSwitcher.test.ts`: кнопка с
  подписью показывает имя активного окружения или «Без окружения»; щелчок открывает меню; выбор вызывает
  `setActiveEnvironment(id)` / `(null)`; при сбое возвращается прежнее имя и есть уведомление; Escape закрывает
  меню без вызова. В тесте дерева: после правки открытого запроса у его строки появляется элемент `.dirty`, а
  `getByRole("treeitem", { name })` находит строку по прежнему имени.
  В `ResponsePanel.test.ts` добавить: «Копировать» вызывает `copyResponseBody` с текущим видом и пишет текст в
  буфер (буфер в тесте — заглушка); при отказе буфера — уведомление об ошибке.
- [ ] **Step 2: Реализовать компоненты и перевести на них существующие экраны.** `Api` и `fakeApi` получают
  `copyResponseBody`.
- [ ] **Step 3: `npm run check`, `npm run test`, `npm run build`** — зелёные; существующие тесты не ослаблены
  (изменённые утверждения перечислить в отчёте с причиной).
- [ ] **Step 4: Commit** — «Часть 3: оформление „Графит“ — набор компонентов и новый вид окна».

---

### Task 3: Создание, переименование, дублирование и удаление в дереве

**Files:**
- Modify: `ui/src/lib/api.ts`, `ui/src/lib/types.ts`, `ui/src/lib/testing/fakeApi.ts`,
  `ui/src/lib/workspace.svelte.ts`, `ui/src/lib/tabs.svelte.ts`,
  `ui/src/components/Sidebar.svelte`, `Tree.svelte`, `TreeRow.svelte`, `ui/src/lib/strings.ts`
- Create: `ui/src/components/TreeMenu.svelte`, `ui/src/components/RenameInput.svelte`
- Test: `ui/src/components/TreeActions.test.ts`

**Поведение:**
- На какую строку действуют команды. Щелчок по папке по-прежнему только раскрывает её и выбор не меняет (это
  закреплено тестом). Поэтому F2, Delete, Shift+F10 и меню действуют на строку с фокусом, а не на выбранную.
  `Tree.svelte` поднимает id строки, на которой фокус был последним, в `Workspace.focusedId`. Цель для кнопки
  «+»: `focusedId`, если его нет — `selectedId`, если нет и его — первая коллекция.
- Кнопка «+» в шапке боковой панели — меню: «Новая коллекция», «Новая папка», «Новый запрос». Папка и запрос
  создаются в целевой папке или коллекции (если цель — запрос, то рядом с ним, в его родителе); если
  коллекций нет — пункты «папка» и «запрос» неактивны с подсказкой «Сначала создайте коллекцию».
  Имена по умолчанию: «Новая коллекция», «Новая папка», «Новый запрос». После создания узел выделяется, родители
  раскрываются, строка сразу в режиме переименования; новый запрос открывается во вкладке.
- Кнопка «…» в строке — только для мыши: `aria-hidden="true"`, `tabindex="-1"`, без подписи, класс `more`
  (иначе она попадёт в доступное имя строки и сломает поиск строк по имени). `TreeMenu` рисуется вне элемента
  `treeitem` и сам останавливает всплытие своих клавиш.
- `RenameInput` останавливает всплытие своих `keydown`, `click`, `dblclick` и `contextmenu`; обработчики строки
  игнорируют события, пришедшие из `input`. Иначе Enter в поле откроет узел, стрелки не будут двигать курсор, а
  Delete при правке имени вызовет вопрос об удалении.
- Меню строки (правая кнопка мыши, клавиша контекстного меню, Shift+F10, кнопка «…» при наведении): у коллекции и
  папки — «Новый запрос», «Новая папка», «Переименовать», «Дублировать», «Удалить» (у коллекции ещё «Экспорт…» —
  пункт добавит Task 5); у запроса — «Переименовать», «Дублировать», «Удалить». Защита от системного меню
  (часть 2) остаётся: в дереве показывается наше меню.
- Переименование: F2 или пункт меню — поле на месте имени, текст выделен; Enter сохраняет, Escape отменяет,
  потеря фокуса сохраняет; пустое имя не сохраняется (поле подсвечено, подсказка). Имя на открытой вкладке
  меняется сразу, её черновик и точка остаются.
- Удаление: Delete на выделенной строке или пункт меню → вопрос «Удалить „<имя>“?» (для папки и коллекции —
  «…и всё содержимое: запросов N»), кнопки «Удалить» (опасная) и «Отмена». После удаления закрываются вкладки
  удалённых узлов без вопроса о сохранении, идущие в них отправки отменяются, тела ответов освобождаются.
- Дублирование: копия появляется сразу под оригиналом и выделяется.
- После каждого действия дерево перечитывается (`tree()`); сбой команды — уведомление, дерево не меняется.

- [ ] **Step 1: Падающие тесты `TreeActions.test.ts`** (fake api хранит дерево в памяти): три вида создания и куда
  встаёт узел; неактивные пункты без коллекций; новая строка в режиме переименования, Enter вызывает
  `renameNode`; Escape не вызывает; пустое имя не сохраняется; F2 на строке; имя открытой вкладки меняется, точка
  остаётся; Delete → вопрос с числом запросов, «Отмена» ничего не делает, «Удалить» вызывает `deleteNode`,
  закрывает вкладки запросов удалённой папки и вызывает `cancelSend` для идущей отправки; дублирование выделяет
  копию; меню открывается правой кнопкой и Shift+F10, системное меню подавлено; сбой `createNode` — уведомление;
  фокус на папке → «Новый запрос» вызывает `createNode(<id папки>, "request", …)`; в режиме переименования
  Delete и стрелка влево не вызывают `deleteNode` и не двигают фокус строк, Enter вызывает `renameNode` и не
  открывает узел; строка по-прежнему находится по точному имени (`getByRole("treeitem", { name })`).
- [ ] **Step 2: Реализовать.**
- [ ] **Step 3: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 4: Commit** — «Часть 3: создание, переименование, дублирование и удаление в дереве».

---

### Task 4: Окружения и глобальные переменные

**Files:**
- Modify: `ui/src/lib/api.ts`, `types.ts`, `testing/fakeApi.ts`, `workspace.svelte.ts`, `tabs.svelte.ts`,
  `ui/src/components/Sidebar.svelte`, `EnvironmentSwitcher.svelte`, `App.svelte`, `ui/src/lib/strings.ts`
- Create: `ui/src/components/EnvironmentList.svelte`, `ui/src/components/EnvironmentEditor.svelte`
- Test: `ui/src/components/Environments.test.ts`

**Модель вкладок (общая для Task 4 и Task 6, меняется в `tabs.svelte.ts`):** вид вкладки —
`"request" | "collection" | "environment" | "history"`; общий интерфейс `{ id, kind, name, dirty,
save(api): Promise<boolean>, dispose(): void }`. Id вкладки: id узла для запросов и коллекций, `"env:" + id` для
окружений, `"history:" + id` для записей истории. `nodeId` — `string | null` (`null` у новых видов, поэтому
удаление узлов из Task 3 их не трогает). У окружения и истории `dirty` всегда `false`; `save()` окружения —
«выполнить отложенное сохранение сейчас», у истории — ничего. Ctrl+S на таких вкладках `saveNodeContent` не
вызывает. `App.svelte` выбирает компонент по виду вкладки. Новые методы: `tabs.openEnvironment(api, id)`
(Task 4), `tabs.openHistory(api, id)` (Task 6).

**Поведение:**
- В шапке боковой панели — `Segmented` «Коллекции / Окружения / История» (раздел «История» наполнит Task 6; до
  неё — `EmptyState` «История появится здесь»). Выбранный раздел в памяти окна, по умолчанию «Коллекции».
- Раздел «Окружения»: первой строкой «Глобальные переменные», ниже окружения по имени; активное отмечено точкой.
  Кнопка «+» создаёт окружение «Новое окружение» и сразу включает переименование. Меню строки: «Сделать
  активным» / «Снять выбор», «Переименовать», «Экспорт…» (Task 5), «Удалить» (нет у глобальных). Удаление — с
  вопросом; если удалено активное — переключатель показывает «Без окружения».
- Щелчок по строке открывает вкладку-редактор: таблица «включена / имя / значение / секрет». Значение секретной
  переменной замаскировано, кнопка «показать». Пустая строка в конце для добавления, удаление строки.
- Сохранение без Ctrl+S (спека, раздел 9): каждая правка сохраняется через 400 мс после последнего нажатия и при
  потере фокуса строкой; точки «не сохранено» у такой вкладки нет; сбой сохранения — уведомление и пометка
  «не сохранено» в шапке редактора, следующая правка повторяет попытку. Отложенное сохранение живёт в объекте
  вкладки (`flush(api): Promise<boolean>`), не в компоненте. Закрытие вкладки: дождаться `flush`, при сбое
  вкладка остаётся. Закрытие окна (`onCloseRequested` в `App.svelte`): сначала `flush` всех вкладок окружений —
  при сбое уведомление, окно остаётся открытым; затем прежняя логика с черновиками. Удаление окружения отменяет
  ожидающее сохранение его вкладки.
- Переключатель окружения и список обновляются после создания, переименования и удаления.

- [ ] **Step 1: Падающие тесты `Environments.test.ts`:** переключение разделов стрелками; список: глобальные
  первыми, активное отмечено; создание → `createEnvironment`, строка в режиме переименования; открытие
  редактора; правка значения → через 400 мс один вызов `saveEnvironment` с полным набором переменных
  (поддельные таймеры), серия быстрых правок — один вызов; потеря фокуса сохраняет сразу; секретное значение
  скрыто и показывается по кнопке; `postman_extra` переменной возвращается без изменений; сбой сохранения —
  пометка и повтор при следующей правке; удаление активного окружения — вопрос, `deleteEnvironment`,
  переключатель на «Без окружения», вкладка закрыта; у глобальных нет пункта «Удалить»; закрытие вкладки с
  отложенным сохранением вызывает его немедленно; запрос на закрытие окна при ожидающем сохранении — один вызов
  `saveEnvironment`, окно закрывается; при отказе `saveEnvironment` — окно остаётся; Ctrl+S на вкладке
  окружения не вызывает `saveNodeContent`. Поддельные таймеры: `vi.useFakeTimers({ toFake: ["setTimeout",
  "clearTimeout"] })` включать только после того, как экран отрисован и найден через `findBy*`; дальше —
  `await vi.advanceTimersByTimeAsync(400)` и синхронные `getBy*` без `waitFor`; в `afterEach` — `vi.useRealTimers()`.
- [ ] **Step 2: Реализовать.**
- [ ] **Step 3: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 4: Commit** — «Часть 3: окружения и глобальные переменные».

---

### Task 5: Экспорт в Postman

**Files:**
- Modify: `ui/src/lib/api.ts`, `testing/fakeApi.ts`, `ui/src/components/TreeMenu.svelte`,
  `EnvironmentList.svelte`, `ui/src/lib/strings.ts`
- Test: `ui/src/components/Export.test.ts`

**Поведение:**
- Пункт «Экспорт…» в меню коллекции, окружения и строки «Глобальные переменные» вызывает `export_collection` /
  `export_environment`. Если у коллекции есть открытые вкладки с несохранёнными правками — перед экспортом
  вопрос: «В коллекции есть несохранённые изменения (вкладок: N). Экспортируется сохранённая версия» с кнопками
  «Сохранить и экспортировать», «Экспортировать как есть», «Отмена».
- Результат-путь → уведомление «Сохранено: <путь>»; `null` → ничего; ошибка → уведомление с текстом.
- Для окружения с секретными переменными перед экспортом предупреждение (спека, раздел 5): «Секретные значения
  попадут в файл открытым текстом» с кнопками «Экспортировать» и «Отмена».

- [ ] **Step 1: Падающие тесты `Export.test.ts`:** экспорт коллекции без правок — один вызов и уведомление с
  путём; отмена (`null`) — без уведомлений; ошибка — уведомление; с несохранёнными вкладками — три ветки вопроса
  (при «Сохранить и экспортировать» сначала `saveNodeContent` для каждой, затем экспорт; сбой сохранения
  останавливает экспорт); окружение с секретом — предупреждение, «Отмена» не вызывает команду; глобальные.
- [ ] **Step 2: Реализовать.**
- [ ] **Step 3: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 4: Commit** — «Часть 3: экспорт коллекций и окружений в Postman».

---

### Task 6: История отправок

**Files:**
- Modify: `ui/src/lib/api.ts`, `types.ts`, `testing/fakeApi.ts`, `tabs.svelte.ts`, `sending.svelte.ts`,
  `ui/src/components/Sidebar.svelte`, `App.svelte`, `ui/src/lib/format.ts`, `ui/src/lib/strings.ts`
- Create: `ui/src/lib/history.svelte.ts`, `ui/src/components/HistoryList.svelte`,
  `ui/src/components/HistoryEntryView.svelte`
- Test: `ui/src/components/History.test.ts`, дополнить `ui/src/lib/format.test.ts`

**Поведение:**
- Раздел «История» в боковой панели: записи новые сверху, сгруппированы по дням («Сегодня», «Вчера», дата вида
  `13.11.2026`). Строка: время `ЧЧ:ММ:СС` по местному времени, метод цветом, имя запроса (а под ним или в
  подсказке — адрес), справа статус цветом и время в мс; у неудачной — текст вида ошибки («отменён», «нет
  соединения», «таймаут»…) вместо статуса. Первые 100 записей; кнопка «Показать ещё» догружает по 100
  (`before_id`).
- Список обновляется при открытии раздела и после каждой отправки: в `Sending.send` сразу после того, как
  `api.sendRequest(...)` завершился (успехом или отказом), во всех ветках, включая отброшенный результат. В
  `cancel()` не перечитывать: оболочка пишет отменённую запись позже, когда команда действительно вернётся.
  Сбой `historyList` — уведомление, результат отправки не трогается.
- Щелчок по записи открывает вкладку только для чтения «История · <имя>» (`tabs.openHistory`). Такая вкладка
  занимает обе области окна — и редактора, и ответа: сверху во всю ширину — когда отправлено, метод и адрес,
  статус, время, размер; ниже слева «Запрос», справа «Ответ»: заголовки таблицей, тело в редакторе кода только
  для чтения. Pretty / Raw у каждого тела: при открытии записи окно один раз вызывает `format_json` для тела;
  успех — Pretty доступен и выбран, отказ — только Raw. Пометки: «Тело обрезано до 1 МБ», «Ответ не текстовый — в истории не
  хранится». Ошибка и предупреждения отправки показаны как в панели ответа.
- В шапке записи кнопка «Открыть запрос» — открывает вкладку исходного запроса; неактивна с подсказкой «Запрос
  удалён», когда `node_exists = false`.
- Кнопка «Очистить историю» в шапке раздела — вопрос «Удалить всю историю? Это нельзя отменить» → `history_clear`;
  открытые вкладки истории закрываются.
- Пустая история — `EmptyState` «Здесь появятся отправленные запросы».
- `format.ts`: `formatClock(iso)` → `ЧЧ:ММ:СС`, `dayLabel(iso, now)` → «Сегодня» / «Вчера» / `ДД.ММ.ГГГГ`.

- [ ] **Step 1: Падающие тесты:** `format.test.ts` — `formatClock` и `dayLabel`; чтобы тест не зависел от часового
  пояса машины, входные значения строить из местного времени: `new Date(2026, 10, 13, 9, 5, 7).toISOString()` →
  `09:05:07`; `now = new Date(2026, 10, 13, 12, 0)`, записи `new Date(2026, 10, 13, 0, 0, 1)` → «Сегодня»,
  `new Date(2026, 10, 12, 23, 59, 59)` → «Вчера», `new Date(2026, 10, 1, 12, 0)` → `01.11.2026`; `History.test.ts` — пустое состояние; группы по дням и порядок; строка успешной
  и неудачной отправки; «Показать ещё» вызывает `historyList(100, <id последней>)` и скрывается, когда пришло
  меньше 100; отправка → Cancel → `historyList` ещё не вызван, после завершения самой команды — вызван один раз; открытие записи вызывает `historyGet` и показывает
  запрос и ответ; Pretty вызывает `formatJson`, не-JSON — только Raw; пометки обрезки и «не текстовый»;
  «Открыть запрос» открывает вкладку узла, а при `node_exists = false` неактивна; вкладка истории не становится
  «несохранённой» и не участвует в вопросе при закрытии окна; очистка — вопрос, `historyClear`, вкладки истории
  закрыты.
- [ ] **Step 2: Реализовать.**
- [ ] **Step 3: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 4: Commit** — «Часть 3: история отправок».

---

### Task 7: Сборка, список проверки для владельца, документы

**Files:**
- Create: `stage1-part3-results.md` — что сделано, числа тестов, все
  **[моё решение]** этого плана, новая зависимость, изменение ядра, что осталось на план 2
- Create: `stage1-part3-owner-checklist.md`
- Modify: `docs/design.md` — раздел 5 (таблица `history`, версия схемы 2),
  раздел 9 (оформление «Графит», раздел «История»), раздел 12 (что сделано, что на план 2)

- [ ] **Step 1: Полный прогон:** `cargo test --workspace`, clippy; в `ui/`: `npm run check`, `npm run test`,
  `npm run build`. Проверить `cargo tree -p rqm-runner -i tauri` (и для остальных `rqm-*`) — пакета нет;
  `git ls-files spike/samples` — пусто.
- [ ] **Step 2: Собрать приложение:** убедиться, что программа не запущена (`tasklist` не показывает
  `requestmaster.exe`; если запущена — не собирать, вернуть `BLOCKED` с просьбой к владельцу закрыть её); в
  `ui/` — `npm run tauri build`. Приложение не запускать. Записать размеры exe и установщика (установщик ≤ 20 МБ).
- [ ] **Step 3: Список проверки для владельца** — простыми словами, по шагам с «что должно получиться»: вид окна
  (сверить с макетом «А · Графит»); создание коллекции, папки, запроса; переименование F2; дублирование;
  удаление с открытой вкладкой; правая кнопка в дереве; окружения — создать, вписать переменные, сделать
  активным, отправить запрос с `{{переменной}}`, удалить активное; глобальные переменные; экспорт коллекции и
  открытие файла в Postman; экспорт окружения с секретом; копирование ответа; история — запись успешной,
  неудачной и отменённой отправки, просмотр записи, «Открыть запрос», очистка; перезапуск — данные и история на
  месте (база старой версии обновилась без потерь).
- [ ] **Step 4: Документ результатов и обновление спеки.**
- [ ] **Step 5: Commit** — «Часть 3: сборка, список проверки и итоги».
