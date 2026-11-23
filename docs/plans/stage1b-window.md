# RequestMaster, этап 1B «Окно: основа» — план реализации

**Goal:** Первое рабочее окно RequestMaster: импортировать коллекцию Postman, увидеть дерево, открыть запрос,
поправить его, нажать Send и увидеть ответ.

**Architecture:** Тонкая оболочка `src-tauri/` (Tauri 2) поверх готового ядра `crates/rqm-*`: вся логика — в
`AppCore` (обычный Rust без типов Tauri, покрыт тестами), команды Tauri только вызывают его. Интерфейс — `ui/`
(Svelte 5 + TypeScript + Vite): компоненты, типизированная обёртка над командами, никакой логики Postman,
переменных и HTTP в TypeScript.

**Tech Stack:** Tauri `=2.12.5` (`tauri-build =2.7.4`), Svelte `5.57.3`, TypeScript `6.0.3`, Vite `8.3.8`,
CodeMirror 6 (`codemirror 6.0.2`, `@codemirror/lang-json 6.0.2`, `@codemirror/state 6.7.6`),
`@tauri-apps/api 2.12.4`, `@tauri-apps/cli 2.12.4` — те же версии, что в `spike/shell`. Тесты интерфейса:
`vitest` + `jsdom` + `@testing-library/svelte` (новые dev-зависимости, закрепить точно). Брать последнюю
стабильную версию, у которой поле `engines` допускает установленный Node (проверять `node --version` и
`npm view <пакет>@<версия> engines`); предупреждений `EBADENGINE` при `npm install` быть не должно.

**Spec:** `docs/design.md` (дальше «спека»). Ядро и его решения —
`docs/plans/stage1a-core.md` и `stage1a-results.md`.

## Что входит и что отложено в часть 3

Входит: импорт файлов Postman (кнопка и перетаскивание) с отчётом; дерево с папками, сворачиванием и фильтром;
вкладки запросов; редактор запроса — метод, URL, Params, Headers, Body (raw и urlencoded), Auth; вкладка
коллекции — переменные и Auth; сохранение по Ctrl+S; Send / Cancel; панель ответа — статус, время, размер,
Body (Pretty / Raw), Headers, Cookies; предупреждения отправки; переключатель окружения; тёмная тема
«Graphite & Signal».

Отложено в часть 3 этапа 1 («Окно: полнота») **[моё решение]** — владелец видит это в плане и может поднять: создание, переименование,
перемещение, удаление и дублирование узлов; перетаскивание в дереве; редактор окружений и globals; экспорт;
редактирование form-data и binary (в 1B показываются только для чтения); вкладка Scripts и Settings запроса;
редактор папки; подсветка и автодополнение `{{var}}`; Preview HTML; поиск в ответе; анимации сверх простых
переходов; виртуализация дерева; запоминание вкладок, черновиков и размеров панелей между запусками;
настройки приложения (таймаут, прокси, SSL — действуют значения по умолчанию); замеры раздела 10 спеки;
строгая CSP для стилей редактора (в 1B — настройка из пробника, итоги этапа 0 §2.4); кнопка «Сохранить в файл»
для ответа больше 20 МБ (в 1B виден только его первый кусок); события прогресса запроса; переключатель
Collections / Environments и сворачивание сайдбара; ручной переключатель раскладки «редактор и ответ».

## Global Constraints

- Ветка: `feature/stage1b-window`, создаётся от `feature/stage1a-core`. В `main`/`master` не коммитить. Push — никогда.
- Сообщения коммитов на русском.
- Приложение и dev-серверы не запускать: ни `tauri dev`, ни `vite`, ни собранный exe. Разрешено: `cargo build`,
  `cargo test`, `cargo clippy`, `npm install`, `npm run check`, `npm run test`, `npm run build`,
  `npm run tauri build`. Окно проверяет владелец по списку из Task 7.
- Сеть: crates.io и npm; Tauri CLI при первой сборке установщика уже скачал NSIS (итоги этапа 0 §6 п. 13) —
  повторная загрузка не ожидается. Тесты ходят только на `127.0.0.1`.
- Версии зависимостей закреплять точно. Rust — в корневом `[workspace.dependencies]`.
- Ядро `crates/rqm-*` в этом плане не меняется. Если без правки ядра задачу не выполнить — остановиться и
  вернуть `BLOCKED` с описанием; исключения перечислены в задачах явно.
- Схема БД не меняется. Новые ключи `app_state` (новых колонок не требуют): `active_environment` — id активного
  окружения, пишет и читает `AppCore`.
- В TypeScript нет разбора Postman-JSON, подстановки переменных, сборки HTTP-запроса и форматирования JSON
  ответа — это делает ядро. В TypeScript допустимы только помощники отображения (дерево из плоского списка,
  форматирование размера и времени).
- Тексты интерфейса: сообщения и подписи — на русском; общепринятые термины HTTP и Postman — на английском
  (`Params`, `Headers`, `Body`, `Auth`, `Send`, `Cancel`, `Pretty`, `Raw`, `Cookies`) **[моё решение]**. Все
  тексты лежат в `ui/src/lib/strings.ts` под английскими ключами; в компонентах строк на русском нет.
- Данные владельца: `spike/samples/` в git не попадает; значения из образцов не печатать и не копировать в
  тесты, фикстуры, отчёты и коммиты.
- Только то, что в задаче: без новых команд, настроек и экранов «на будущее».
- Rust: публичное — с doc-комментарием на английском; `unwrap()`/`expect()` вне тестов — только с пояснением.
  Каждая задача с Rust заканчивается зелёными `cargo test -p requestmaster` и
  `cargo clippy -p requestmaster --all-targets -- -D warnings`.
- UI: каждая задача с UI заканчивается зелёными `npm run check`, `npm run test`, `npm run build` (в `ui/`).
- Доступность: все действия доступны с клавиатуры, у элементов управления есть подписи (`aria-label` там, где
  нет текста), фокус виден.

## Контракт команд (общий для всех задач)

Команды Tauri — имена в `snake_case`, аргументы и ответы — JSON с ключами в `snake_case` (у команд стоит
`#[tauri::command(rename_all = "snake_case")]`; интерфейс передаёт в `invoke` ключи `send_id`, `request_id`,
`body_id` и т.д.). Все команды, кроме `cancel_send`, асинхронные (`async fn`, возвращают `Result<_, String>`),
чтобы разбор большой коллекции или чтение тела не замораживали окно; замок хранилища через `.await` не
удерживается. Ошибка любой команды — строка с понятным сообщением на
английском; интерфейс показывает её в уведомлении.

```text
tree() -> TreeRow[]                         TreeRow = { id, parent_id: string|null, kind: "collection"|"folder"|"request",
                                                        name, sort_order: number, method: string|null }
get_node(id) -> NodeDto                     NodeDto = { id, parent_id, kind, name, content }
    // content = JSON из NodeContent::to_json БЕЗ верхнего ключа postman_extra (см. ниже)
save_node_content(id, content) -> null      // content того же вида; вид берётся из хранилища
import_files(files: {name, text}[]) -> ImportResult[]
    ImportResult = { file_name, kind: "collection"|"environment"|"globals"|"error", name: string|null,
                     root_id: string|null, report: ReportItem[], conflicts: string[], error: string|null }
    ReportItem   = { kind: "unsupported_auth"|"setting_not_applied"|"has_scripts"|"file_reference",
                     node_id, node_path, detail }
    // globals с конфликтами не применяются: kind = "globals", conflicts непустой; тогда UI спрашивает и вызывает:
import_globals(text, policy: "take_incoming"|"keep_current") -> null
environments() -> { id, name }[]            // без globals, по имени
active_environment() -> string|null
set_active_environment(id: string|null) -> null
send_request(send_id, request_id, draft: object|null) -> SendResult
    SendResult = { response: ResponseDto|null, error: SendErrorDto|null, warnings: Warning[] }
    ResponseDto = { status, status_text, http_version, headers: [string,string][], cookies: CookieDto[],
                    redirects: { status, method, url, location }[], ttfb_ms, total_ms, wire_size, decoded_size,
                    decode_error: string|null, sent: { method, url, headers: [string,string][] },
                    body: { id, total: number, truncated: boolean, json: boolean } }
    CookieDto = { name, value, domain, path, expires_unix: number|null, secure, http_only }
    SendErrorDto = { kind: "cancelled"|"timeout"|"invalid_url"|"connect"|"tls"|"too_many_redirects"|"io"|
                           "unconfirmed_files"|"other", message, fields: string[] }
    Warning = { kind: "scripts_not_executed"|"unsupported_auth"|"body_pruned"|"unresolved_variables", items: string[] }
cancel_send(send_id) -> null                // неизвестный send_id — не ошибка
response_body(body_id, pretty: boolean) -> сырые байты (tauri::ipc::Response)   // по правилам rqm_runner::body_view
release_body(body_id) -> null               // удаляет файл тела; неизвестный id — не ошибка
format_json(text) -> string                 // доводка 2026-11-13: отступы для JSON тела запроса по правилам
                                            // rqm_runner::format_json (числа, порядок и повторы ключей — как
                                            // написано; один BOM в начале отбрасывается); не JSON — ошибка
                                            // "not valid JSON"
```

Верхний `postman_extra` узла (исходный фрагмент Postman, у запроса — вместе с сохранёнными примерами ответов)
в окно не передаётся: проход через JavaScript переставляет числовые ключи и портит большие числа, а экспорт
обязан остаться без потерь. `get_node` его вырезает, `save_node_content` подставляет обратно из сохранённого
узла, `send_request` разбирает черновик без него (для отправки он не нужен). `postman_extra` строк
(`Param`, `Variable`, части form-data) остаётся в `content`, интерфейс возвращает его без изменений.

`send_id` создаёт интерфейс (`crypto.randomUUID()`), чтобы Cancel мог сослаться на отправку, которая ещё не
вернулась. `body.id` создаёт ядро оболочки; файл тела живёт, пока интерфейс не вызовет `release_body` или
приложение не закроется.

## Review Focus

1. **Cancel во время долгого запроса** — кнопка снова становится Send, показывается «Отменено», а не ошибка сети;
   поздний ответ отменённой отправки не подменяет панель ответа (отмена в ядре — тест в Task 2, поздний ответ —
   тест в Task 6).
2. **Два Send подряд в одной вкладке** — в панели остаётся ответ последней отправки; файл тела предыдущей
   освобождается (тест в Task 6).
3. **Несохранённые правки** — Send отправляет черновик, а не сохранённую версию; закрытие вкладки с черновиком
   спрашивает подтверждение (тесты в Task 5 и Task 6).
4. **Импорт не того файла** (картинка, не-JSON, коллекция v2.0) — понятное сообщение по каждому файлу, остальные
   файлы той же пачки импортируются (тесты в Task 1 и Task 4).
5. **Большой или не-текстовый ответ** — обрезанный ответ помечен «показаны первые 256 КБ из N»; бинарные байты
   не ломают панель (тест в Task 6).

---

### Task 1: Оболочка `src-tauri` и `AppCore` — дерево, узлы, импорт, окружения

**Files:**
- Modify: `Cargo.toml` (корень) — `members = ["crates/*", "src-tauri"]`; в `[workspace.dependencies]` добавить
  `tauri = "=2.12.5"`, `tauri-build = "=2.7.4"`
- Create: `src-tauri/Cargo.toml` (пакет `requestmaster`, lib `requestmaster_lib`, `crate-type = ["rlib"]`, bin
  `requestmaster`), `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`,
  `src-tauri/icons/` (скопировать из `spike/shell/src-tauri/icons/`), `src-tauri/src/main.rs`,
  `src-tauri/src/lib.rs`, `src-tauri/src/core.rs`, `src-tauri/src/dto.rs`, `src-tauri/src/commands.rs`
- Modify: `.gitignore` — добавить `src-tauri/gen/` и `src-tauri/permissions/autogenerated/` (их создаёт сборка;
  папку `permissions/` руками не создавать)
- Test: `src-tauri/tests/core_data.rs`

**Interfaces:**
- Consumes: `rqm-model`, `rqm-postman`, `rqm-store` (читать их `src/lib.rs` — API уже реализован).
- Produces:

```rust
/// Everything the window needs, without any Tauri type. One instance per application.
pub struct AppCore { /* Mutex<Store>, bodies_dir, … (Task 2 adds the engine, the jar and the send registry) */ }
pub struct CoreConfig { pub db_path: Option<std::path::PathBuf> /* None = in memory */, pub bodies_dir: std::path::PathBuf }
impl AppCore {
    pub fn new(config: CoreConfig) -> Result<Self, String>;
    pub fn tree(&self) -> Result<Vec<dto::TreeRow>, String>;
    pub fn get_node(&self, id: &str) -> Result<dto::NodeDto, String>;
    pub fn save_node_content(&self, id: &str, content: serde_json::Value) -> Result<(), String>;
    pub fn import_files(&self, files: Vec<dto::ImportFile>) -> Vec<dto::ImportResult>;
    pub fn import_globals(&self, text: &str, policy: &str) -> Result<(), String>;
    pub fn environments(&self) -> Result<Vec<dto::EnvSummary>, String>;
    pub fn active_environment(&self) -> Result<Option<String>, String>;
    pub fn set_active_environment(&self, id: Option<&str>) -> Result<(), String>;
}
pub const ACTIVE_ENVIRONMENT_KEY: &str = "active_environment";
```

  DTO в `dto.rs` — по «Контракту команд», `Serialize`/`Deserialize`. Команды в `commands.rs` — по одной на
  метод, только вызов `AppCore` из `tauri::State`.

**Правила:**
- Команды и права (как в `spike/shell/src-tauri`): список имён команд — одна константа
  `pub const COMMANDS: &[&str]` в `src-tauri/src/lib.rs`; `build.rs` передаёт те же имена в
  `tauri_build::AppManifest::commands`, `capabilities/default.json` содержит для каждой `allow-<имя-через-дефис>`
  (`allow-tree`, `allow-get-node`, `allow-save-node-content`, `allow-import-files`, `allow-import-globals`,
  `allow-environments`, `allow-active-environment`, `allow-set-active-environment`). Команда, которой нет в
  capability, в окне молча не работает, а тесты этого не видят — поэтому совпадение списков проверяет тест.
- `tauri.conf.json`: `frontendDist: "../ui/dist"`, `productName: "RequestMaster"`, `identifier: "kg.nsk.requestmaster"`, окно `main`
  1280×800, минимум 960×600, заголовок `RequestMaster`, системная рамка; `dragDropEnabled: false` (чтобы
  перетаскивание файлов обрабатывал интерфейс); CSP и `dangerousDisableAssetCspModification` — как в
  `spike/shell/src-tauri/tauri.conf.json`; `beforeBuildCommand: "npm --prefix ../ui run build"` заменить на
  рабочий вариант для этой машины (в пробнике `--prefix` падал — итоги этапа 0 §6 п. 14): использовать
  `{"script": "npm run build", "cwd": "../ui"}`; `bundle.targets: ["nsis"]`.
- `main.rs`: создаёт `AppCore` с `db_path = Store::default_path()`, `bodies_dir = <temp>/RequestMaster/bodies`
  (папка очищается при запуске), регистрирует команды. Ошибка создания — окно с сообщением не делать, завершить
  процесс с кодом 1 и текстом в stderr.
- `save_node_content`: вид узла читается из хранилища, `content` разбирается `NodeContent::from_json`; неверный
  JSON → ошибка, в хранилище ничего не меняется.
- `import_files`: каждый файл независим. Не JSON / неизвестный формат / v2.0 → `kind: "error"` с сообщением
  (`not a JSON file`, `unknown format`, `Collection v2.0 is not supported yet`). Коллекция → `insert_nodes`,
  `root_id`, отчёт. Окружение → `save_environment`. Globals: если `globals_conflicts` пуст — слить
  (`merge_globals`, `TakeIncoming`) и сохранить; иначе вернуть `conflicts` и ничего не менять.
- `import_globals`: то же слияние с выбранной политикой; неизвестная политика → ошибка.
- `set_active_environment`: `None` удаляет выбор (пишется значение `null`); id несуществующего окружения → ошибка.
  `active_environment` возвращает `None`, если сохранённое окружение уже удалено.
- Все методы берут `Mutex<Store>` на время одной операции; отравленный mutex → ошибка `storage is unavailable`.

- [ ] **Step 1: Создать пакет и конфигурацию.** Создать локально `ui/dist/index.html` с одной строкой
  (`<!doctype html><title>RequestMaster</title>`): папка `frontendDist` нужна макросу `generate_context!` при любой
  сборке пакета. `dist/` уже в `.gitignore`, файл в git не добавлять; Task 3 заменит его результатом
  `npm run build`. В документ результатов (Task 7) записать: на свежем клоне сначала `npm run build` в `ui/`.
- [ ] **Step 2: Написать падающие тесты `src-tauri/tests/core_data.rs`** (хранилище в памяти, синтетические
  JSON прямо в тесте):
  - `import_collection_then_tree_and_node`: импорт коллекции v2.1 с папкой и двумя запросами → `kind` =
    `"collection"`, `root_id` есть; `tree()` — 4 строки, у запросов `method`; `get_node(запрос).content.method`
    совпадает.
  - `import_reports_each_file_independently`: пачка из `[картинка-как-текст, коллекция v2.0, верная коллекция,
    окружение]` → виды `error`, `error`, `collection`, `environment`; сообщения ошибок как в правилах; в дереве
    одна коллекция; `environments()` — одно окружение.
  - `second_import_appends_collection`: два импорта подряд → две корневые строки с `sort_order` 0 и 1.
  - `save_node_content_round_trip_and_rejects_garbage`: изменить `url` запроса и сохранить → `get_node` отдаёт
    новое; `save_node_content(id, json!("x"))` → `Err`, содержимое прежнее; неизвестный id → `Err`.
  - `postman_extra_never_reaches_the_window_and_survives_save`: коллекция, у запроса которой есть сохранённый
    пример ответа (`response`) и неизвестный ключ → в `get_node(...).content` нет ключа `postman_extra`, но у
    заголовка с `description` его собственный `postman_extra` есть; после `save_node_content` с изменённым URL
    содержимое узла в хранилище (прочитать через `rqm_store::Store`, открыв тот же файл базы во временной
    папке) по-прежнему содержит исходный `postman_extra` с `response`.
  - `commands_are_listed_everywhere`: каждое имя из `COMMANDS` есть в `capabilities/default.json` как
    `allow-<имя-через-дефис>`, лишних `allow-*` нет; `COMMANDS` содержит ровно 8 имён этой задачи.
  - `globals_without_conflicts_merge_and_conflicts_wait_for_choice` (значение проверяется через сам импорт —
    конфликт считается по сохранённому значению): файл 1 применяется (`conflicts` пуст); файл 2 с другим
    значением той же переменной → `conflicts == ["имя"]`; повторный импорт файла 2 снова даёт тот же конфликт
    (значение не менялось); после `import_globals(text2, "take_incoming")` импорт файла 2 — без конфликтов, а
    файла 1 — `conflicts == ["имя"]`; после `import_globals(text1, "keep_current")` файл 2 по-прежнему без
    конфликтов; `import_globals(text1, "nope")` → `Err`.
  - `active_environment_rules`: изначально `None`; установить id импортированного окружения → возвращается он;
    установить несуществующий → `Err`; `set_active_environment(None)` → `None`.
  - `report_items_use_contract_names`: коллекция с `auth` типа `awsv4` и скриптом → в отчёте виды
    `"unsupported_auth"` и `"has_scripts"` (проверить сериализованный JSON).
- [ ] **Step 3: `cargo test -p requestmaster`** — ошибка компиляции.
- [ ] **Step 4: Реализовать `AppCore`, DTO, команды, `main.rs`.**
- [ ] **Step 5: `cargo test -p requestmaster` и clippy** — 9 passed.
- [ ] **Step 6: Commit** — «Этап 1B: оболочка Tauri и AppCore — дерево, узлы, импорт, окружения».

---

### Task 2: `AppCore` — отправка, отмена, тело ответа

**Files:**
- Modify: `src-tauri/src/core.rs`, `src-tauri/src/dto.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`,
  `src-tauri/src/lib.rs` (`COMMANDS` + 4 имени), `src-tauri/build.rs`, `src-tauri/capabilities/default.json`
  (`allow-send-request`, `allow-cancel-send`, `allow-response-body`, `allow-release-body`),
  `src-tauri/tests/core_data.rs` (в `commands_are_listed_everywhere` — ровно 12 имён), `src-tauri/Cargo.toml`
  (зависимости `rqm-http`, `rqm-runner`, `tokio`, `tokio-util`; dev: `wiremock`, `tempfile`)
- Create: `src-tauri/src/send.rs`
- Test: `src-tauri/tests/core_send.rs`

**Interfaces:**
- Consumes: Task 1; `rqm_runner::{load_send_input, send, body_view, load_cookies, save_cookies, AppSettings,
  SendWarning, SendError, PrepareError}`, `rqm_http::{ReqwestEngine, EngineConfig, CookieJar, HttpError,
  response_cookies}`.
- Produces:

```rust
impl AppCore {
    pub async fn send_request(&self, send_id: &str, request_id: &str, draft: Option<serde_json::Value>) -> Result<dto::SendResult, String>;
    pub fn cancel_send(&self, send_id: &str);
    /// Bytes by the rules of `rqm_runner::body_view`.
    pub fn response_body(&self, body_id: &str, pretty: bool) -> Result<Vec<u8>, String>;
    pub fn release_body(&self, body_id: &str);
}
```

**Правила:**
- `AppCore::new` создаёт `CookieJar` из хранилища (`load_cookies`) и `ReqwestEngine` с
  `user_agent = "RequestMaster/<версия пакета>"` и `body_dir = bodies_dir`.
- `send_request`: под замком хранилища собрать `SendInput` (`load_send_input` с активным окружением и
  `AppSettings::default()`), замок отпустить, зарегистрировать токен отмены под `send_id`, выполнить
  `rqm_runner::send`, снять регистрацию, сохранить cookies (`save_cookies`). `draft` — JSON содержимого запроса;
  неразбираемый → `Err`. `Err` возвращается только за сбой самой оболочки (нет узла, плохой черновик); сбой сети
  и подготовки — это `SendResult.error`.
- Соответствие ошибок: `HttpError::Cancelled` → `cancelled`, `Timeout` → `timeout`, `InvalidUrl` и
  `PrepareError::InvalidUrl` → `invalid_url`, `Connect` → `connect`, `Tls` → `tls`, `TooManyRedirects` →
  `too_many_redirects`, `Io` → `io`, `PrepareError::UnconfirmedFiles` → `unconfirmed_files` (с `fields`),
  остальное → `other`. `message` — текст ошибки ядра.
- Предупреждения: `ScriptsNotExecuted` → `scripts_not_executed` (имена узлов), `UnsupportedAuth` →
  `unsupported_auth` (тип), `BodyPruned` → `body_pruned` (метод), `UnresolvedVariables` →
  `unresolved_variables` (имена).
- Успех: файл тела регистрируется под новым `body.id`; `body.total` = размер файла, `truncated` и `json` —
  из `body_view(path, true)` (`json` = признак `pretty` у нетронутого тела; для обрезанного — `false`).
  `cookies` — `response_cookies`. Время — в миллисекундах, дробное (`f64`).
- `cancel_send` отменяет токен; повторная отмена и неизвестный id — не ошибка. После отмены `send_request`
  возвращает `error.kind = "cancelled"`.
- `release_body` удаляет файл и запись; `response_body` для неизвестного id → `Err("response body is gone")`.

- [ ] **Step 1: Написать падающие тесты `src-tauri/tests/core_send.rs`** (`#[tokio::test]`, сервер `wiremock`,
  хранилище в памяти (кроме `cookies_survive_restart`: там `db_path` — файл во временной папке, первый `AppCore`
  уничтожается до создания второго), `bodies_dir` во временной папке; коллекция создаётся импортом синтетического JSON;
  адреса `http://127.0.0.1:…`; чтобы тесты не зависели от системного прокси машины, в тестах задать переменную
  окружения `NO_PROXY=127.0.0.1` до создания `AppCore`):
  - `send_returns_response_and_body`: сервер отвечает `200` с JSON-телом и заголовком `set-cookie` →
    `response.status == 200`, `body.json == true`, `body.truncated == false`, `cookies.len() == 1`,
    `response_body(id, true)` — отформатированный JSON, `response_body(id, false)` — тело как пришло;
    после `release_body` файл на диске исчез и `response_body` → `Err`.
  - `draft_is_sent_instead_of_saved`: сохранённый URL ведёт на закрытый порт, черновик — на сервер → `200`.
  - `environment_variables_are_applied`: URL `{{base}}/x`, активное окружение с `base` → запрос дошёл.
  - `warnings_and_network_error`: запрос со скриптом на закрытый порт → `error.kind == "connect"`,
    `warnings[0].kind == "scripts_not_executed"`, `response == null`.
  - `cancel_stops_a_slow_request`: сервер с задержкой 5 с; через 100 мс `cancel_send` → результат за < 2 с,
    `error.kind == "cancelled"`; в `bodies_dir` нет файлов.
  - `unconfirmed_file_blocks_send`: запрос с form-data файлом из импорта → `error.kind == "unconfirmed_files"`,
    `fields == ["<имя поля>"]`.
  - `cookies_survive_restart`: после ответа с `set-cookie` создать второй `AppCore` на том же файле базы →
    следующий запрос на тот же сервер несёт заголовок `cookie`.
  - `bad_draft_and_unknown_request_are_errors`: `draft = json!(5)` → `Err`; неизвестный `request_id` → `Err`.
- [ ] **Step 2: `cargo test -p requestmaster`** — ошибка компиляции.
- [ ] **Step 3: Реализовать отправку, отмену, тело; добавить команды.** `response_body` в команде возвращает
  `tauri::ipc::Response::new(bytes)`. `send_request` — `async` команда.
- [ ] **Step 4: `cargo test -p requestmaster` и clippy** — 9 + 8 passed.
- [ ] **Step 5: Commit** — «Этап 1B: отправка запроса, отмена и тело ответа в AppCore».

---

### Task 3: Основа интерфейса — проект `ui/`, тема, каркас окна, обёртка команд

**Files:**
- Create: `ui/package.json`, `ui/index.html`, `ui/vite.config.ts`, `ui/svelte.config.js`, `ui/tsconfig.json`,
  `ui/vitest.config.ts`, `ui/src/test-setup.ts`, `ui/src/main.ts`, `ui/src/App.svelte`, `ui/src/styles/tokens.css`,
  `ui/src/styles/base.css`, `ui/src/lib/api.ts`, `ui/src/lib/types.ts`, `ui/src/lib/strings.ts`,
  `ui/src/lib/format.ts`, `ui/src/lib/testing/fakeApi.ts`, `ui/src/components/Splitter.svelte`,
  `ui/src/components/Toasts.svelte`, `ui/src/lib/toasts.svelte.ts`
- Test: `ui/src/lib/format.test.ts`, `ui/src/styles/contrast.test.ts`, `ui/src/App.test.ts`
- Modify: `.gitignore` — убедиться, что `ui/node_modules/` и `ui/dist/` игнорируются

**Interfaces:**
- Consumes: «Контракт команд».
- Produces:
  - `ui/src/lib/types.ts` — типы TypeScript точно по «Контракту команд» плюс типы содержимого узлов, зеркало
    JSON `rqm-model` (`RequestContent`, `CollectionContent`, `FolderContent`, `Param`, `Variable`, `Auth`,
    `Body`…). Источник истины — `crates/rqm-model/src/*.rs` (читать сериализованные имена полей и вариантов там,
    включая `skip_serializing_if`: отсутствующее поле = значение по умолчанию).
  - `ui/src/lib/api.ts` — `export interface Api { tree(): Promise<TreeRow[]>; getNode(id): Promise<NodeDto>;
    saveNodeContent(id, content): Promise<void>; importFiles(files): Promise<ImportResult[]>;
    importGlobals(text, policy): Promise<void>; environments(): Promise<EnvSummary[]>;
    activeEnvironment(): Promise<string | null>; setActiveEnvironment(id: string | null): Promise<void>;
    sendRequest(sendId, requestId, draft): Promise<SendResult>; cancelSend(sendId): Promise<void>;
    responseBody(bodyId, pretty): Promise<Uint8Array>; releaseBody(bodyId): Promise<void>; }` и
    `export const tauriApi: Api` (через `invoke` из `@tauri-apps/api/core`, ключи аргументов — в `snake_case`).
    `invoke` для сырых байтов отдаёт `ArrayBuffer`, поэтому
    `responseBody: async (bodyId, pretty) => new Uint8Array(await invoke<ArrayBuffer>('response_body', { body_id: bodyId, pretty }))`.
    Компоненты получают `Api` из контекста Svelte (`getContext('api')`), а не импортируют `tauriApi` напрямую:
    в приложении — `mount(App, { target, context: new Map([['api', tauriApi]]) })` в `main.ts`, в тестах —
    `render(App, { context: new Map([['api', createFakeApi()]]) })`.
  - `ui/vitest.config.ts` — отдельный от `vite.config.ts`, поэтому плагины подключаются заново:
    `defineConfig({ plugins: [svelte(), svelteTesting()], test: { environment: 'jsdom', include: ['src/**/*.test.ts'], setupFiles: ['src/test-setup.ts'] } })`,
    `svelteTesting` — из `@testing-library/svelte/vite` (без него берётся серверная сборка Svelte и `mount`
    падает). Тест, который сам использует руны, называется `*.svelte.test.ts`. `ui/tsconfig.json` —
    `"types": ["vite/client"]`.
  - `ui/src/lib/testing/fakeApi.ts` — `createFakeApi(seed?)`: реализация `Api` в памяти для тестов компонентов
    (дерево из двух папок и трёх запросов, ответы `sendRequest` задаются тестом). В сборку приложения не входит.
  - `format.ts` — `formatBytes(n)` (`0 Б`, `512 Б`, `1,5 КБ`, `20,8 МБ`), `formatMs(n)` (`0 мс`, `123 мс`,
    `1,24 с`), `statusClass(status)` → `"ok" | "redirect" | "client" | "server"`.
  - `toasts.svelte.ts` — `toasts.error(text)`, `toasts.info(text)`; `Toasts.svelte` показывает и убирает через 6 с.
  - `App.svelte` — каркас: сайдбар слева (280 px, меняется перетаскиванием `Splitter`, от 200 до 480 px),
    основная зона справа с областями «вкладки», «редактор», «ответ»; при ширине окна ≥ 1280 px редактор и ответ
    рядом, иначе друг под другом. Области пока пустые с подписью-заглушкой из `strings.ts`.

**Визуальный стиль «Graphite & Signal»** (спека, раздел 9) — значения токенов **[моё решение]**, все цвета только
через CSS-переменные в `tokens.css`:

```css
:root {
  --bg: #14171c; --surface: #1b1f26; --surface-2: #232831; --border: #2c323d;
  --text: #dde2ea; --text-muted: #98a1b0; --accent: #2fc7c0; --accent-contrast: #0b1416;
  --method-get: #4cc38a; --method-post: #e5a93b; --method-put: #5b9cf5; --method-patch: #a98bf5;
  --method-delete: #f07178; --method-other: #98a1b0;
  --status-ok: #4cc38a; --status-redirect: #5b9cf5; --status-client: #e5a93b; --status-server: #f07178;
  --danger: #f07178;
  --font-ui: "Segoe UI Variable", "Segoe UI", Inter, system-ui, sans-serif;
  --font-mono: "JetBrains Mono", "Cascadia Mono", Consolas, monospace;
  --font-size: 13px; --row-height: 28px; --radius: 4px; --radius-popup: 8px; --space: 4px;
  --dur-1: 83ms; --dur-2: 120ms; --dur-3: 167ms; --dur-4: 250ms; --ease: cubic-bezier(0, 0, 0, 1);
}
```

  Шрифт JetBrains Mono в приложение в 1B не упаковывается (используется, если установлен, иначе запасные).
  Тени — только у всплывающих элементов; границы 1 px; фокус — обводка `--accent` 2 px.
  `@media (prefers-reduced-motion: reduce)` обнуляет длительности.

- [ ] **Step 1: Создать проект** (`npm install` выполнять внутри `ui/`). Скрипты `package.json`: `build`
  (`vite build`), `check` (`svelte-check --tsconfig ./tsconfig.json`), `test` (`vitest run`), `tauri` (`cd .. && tauri` — Tauri CLI ищет `tauri.conf.json` только в текущей
  папке и ниже, а `src-tauri` лежит рядом с `ui/`; npm оставляет `ui/node_modules/.bin` в PATH).
- [ ] **Step 2: Написать падающие тесты:**
  - `format.test.ts`: `formatBytes(0) = "0 Б"`, `(512) = "512 Б"`, `(1536) = "1,5 КБ"`,
    `(21_757_952) = "20,8 МБ"`; `formatMs(0) = "0 мс"`, `(123.4) = "123 мс"`, `(1240) = "1,24 с"`;
    `statusClass(204) = "ok"`, `(302) = "redirect"`, `(404) = "client"`, `(503) = "server"`.
  - `contrast.test.ts`: читает `tokens.css` (`import css from './tokens.css?raw'`), считает контраст по WCAG для пар `--text`/`--bg`,
    `--text`/`--surface`, `--text-muted`/`--surface`, каждого `--method-*` и `--status-*` на `--surface`,
    `--accent-contrast`/`--accent` — каждое ≥ 4,5.
  - `App.test.ts` (Testing Library, `createFakeApi()` в контексте): отрисовались области с ролями `navigation`
    (сайдбар) и `main`; у разделителя роль `separator`, стрелки влево/вправо меняют ширину сайдбара на 16 px и
    не выходят за 200–480.
- [ ] **Step 3: `npm run test`** — тесты падают.
- [ ] **Step 4: Реализовать.**
- [ ] **Step 5: `npm run check`, `npm run test`, `npm run build`** — всё зелёное; затем
  `cargo build -p requestmaster` — успех (оболочка видит `ui/dist`).
- [ ] **Step 6: Commit** — «Этап 1B: основа интерфейса — проект ui, тема, каркас окна».

---

### Task 4: Сайдбар — импорт, отчёт импорта, дерево

**Files:**
- Create: `ui/src/lib/tree.ts`, `ui/src/lib/workspace.svelte.ts`, `ui/src/components/Sidebar.svelte`,
  `ui/src/components/Tree.svelte`, `ui/src/components/TreeRow.svelte`, `ui/src/components/ImportReport.svelte`,
  `ui/src/components/Dialog.svelte`, `ui/src/components/EnvironmentSwitcher.svelte`
- Modify: `ui/src/App.svelte`, `ui/src/lib/strings.ts`
- Test: `ui/src/lib/tree.test.ts`, `ui/src/components/Sidebar.test.ts`

**Interfaces:**
- Consumes: Task 3 (`Api`, типы, `toasts`, `createFakeApi`).
- Produces:
  - `tree.ts` — `buildTree(rows: TreeRow[]): TreeNode[]` (`TreeNode = TreeRow & { children: TreeNode[] }`,
    потомки по `sort_order`); `visibleRows(tree, expanded: Set<string>, filter: string): { node, depth }[]` —
    плоский список для отрисовки: без фильтра — по раскрытым узлам; с фильтром (без учёта регистра, по имени) —
    совпавшие узлы вместе со всеми предками, всё раскрыто.
  - `workspace.svelte.ts` — общее состояние окна (руны Svelte 5): `rows`, `expanded`, `filter`, `selectedId`,
    `environments`, `activeEnvironmentId`; методы `load(api)`, `toggle(id)`, `select(id)`,
    `setEnvironment(api, id)`; колбэк `onOpen(id)` — его в Task 5 подключат вкладки.
  - `Sidebar.svelte` — кнопка «Импорт» (скрытый `<input type="file" multiple accept=".json">`), приём
    перетаскивания файлов на сайдбар, поле фильтра, дерево.
  - `ImportReport.svelte` — диалог после импорта: по каждому файлу — что импортировано или текст ошибки; ниже
    записи отчёта, сгруппированные по виду, с путём узла.
  - `EnvironmentSwitcher.svelte` — выпадающий список в правом верхнем углу: «Без окружения» и окружения.

**Поведение:**
- При запуске: `tree()`, `environments()`, `activeEnvironment()`. Пустое дерево — заглушка «Перетащите сюда файл
  коллекции Postman или нажмите „Импорт“».
- Импорт: файлы читаются как текст (`File.text()`), все вместе уходят в `importFiles`; затем дерево и окружения
  перечитываются, корни импортированных коллекций раскрываются, показывается `ImportReport`. Для результата
  globals с непустым `conflicts` — диалог со списком имён и кнопками «Взять из файла», «Оставить текущие»,
  «Отмена» → `importGlobals`.
- Строка дерева: высота `--row-height`, отступ 16 px на уровень; у папки и коллекции — стрелка сворачивания, у
  запроса — метод цветным моноширинным текстом (`GET` зелёный и т.д.; неизвестный метод — `--method-other`,
  длинный обрезается до 6 символов). Наведение и выделение — без анимации.
- Клавиатура: дерево — `role="tree"`, строки — `treeitem` с `aria-expanded`/`aria-level`; стрелки вверх/вниз
  двигают фокус, вправо/влево раскрывают/сворачивают, Enter открывает (`onOpen`). Щелчок по запросу и по
  коллекции — `select` + `onOpen`; по папке — раскрыть/свернуть.
- Перетаскивание мимо сайдбара: слушатели `dragover` и `drop` на `window` всегда вызывают `preventDefault()` —
  иначе окно уйдёт на страницу брошенного файла и интерфейс пропадёт; импорт выполняется, только когда цель
  внутри сайдбара.
- `Dialog.svelte` — `<div role="dialog" aria-modal="true">` с ловушкой фокуса и закрытием по Escape (не
  `<dialog>.showModal()`: в тестовой среде его может не быть). Если в тестовой среде нет `File.prototype.text`,
  добавить полифил через `FileReader` в `ui/src/test-setup.ts`.
- Смена окружения вызывает `setActiveEnvironment`; сбой — уведомление, выбор возвращается к прежнему.

- [ ] **Step 1: Написать падающие тесты:**
  - `tree.test.ts`: `buildTree` собирает вложенность и порядок из перемешанного списка; узел с неизвестным
    родителем не теряется, а попадает в корень; `visibleRows` без фильтра показывает только раскрытое; с фильтром
    `"get"` находит запрос в свёрнутой папке вместе с предками; пустой результат — пустой список.
  - `Sidebar.test.ts` (fake api): пустое дерево → видна заглушка; после «импорта» (тест подаёт файл в `input`) —
    вызван `importFiles` с именем и текстом файла, дерево перечитано, диалог отчёта показывает имя коллекции и
    запись вида «Скрипты» с путём; файл с ошибкой показан своим сообщением рядом с успешным; конфликт globals →
    диалог выбора, «Взять из файла» вызывает `importGlobals(text, "take_incoming")`; ввод в фильтр оставляет
    совпавшие строки; Enter на запросе вызывает `onOpen` с его id; стрелка вправо раскрывает папку; у строки
    `GET` метод имеет класс цвета `method-get`; `drop` и `dragover` на `document.body` дают
    `defaultPrevented === true`, а `importFiles` при этом не вызван; диалог закрывается по Escape.
- [ ] **Step 2: `npm run test`** — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «Этап 1B: сайдбар — импорт с отчётом, дерево, выбор окружения».

---

### Task 5: Вкладки и редакторы запроса и коллекции

**Files:**
- Create: `ui/src/lib/tabs.svelte.ts`, `ui/src/components/TabBar.svelte`, `ui/src/components/RequestEditor.svelte`,
  `ui/src/components/UrlBar.svelte`, `ui/src/components/KeyValueTable.svelte`,
  `ui/src/components/BodyEditor.svelte`, `ui/src/components/AuthEditor.svelte`,
  `ui/src/components/CollectionEditor.svelte`, `ui/src/components/CodeEditor.svelte`,
  `ui/src/lib/testing/CodeEditorStub.svelte` (`<textarea>` с теми же свойствами `value`, `readonly`, `onchange`),
  `ui/src/lib/hotkeys.ts`
- Modify: `ui/src/App.svelte`, `ui/src/lib/workspace.svelte.ts`, `ui/src/lib/strings.ts`, `ui/src/lib/types.ts`
- Test: `ui/src/lib/tabs.test.ts`, `ui/src/components/RequestEditor.test.ts`,
  `ui/src/components/CollectionEditor.test.ts`

**Interfaces:**
- Consumes: Task 3–4.
- Produces:
  - `tabs.svelte.ts` — `tabs.open(api, nodeId)` (уже открытый узел — просто активируется; папка не открывается),
    `tabs.close(tabId)`, `tabs.activate(tabId)`, `tabs.active`, `tabs.list`; у вкладки: `nodeId`, `kind`, `name`,
    `saved` (содержимое из хранилища), `draft` (редактируемая копия), `dirty` (глубокое сравнение `draft` и
    `saved`). Содержимое из `get_node` один раз проходит `normalizeContent(kind, content)` (добавить в
    `types.ts`): подставляются все значения по умолчанию, которые ядро не пишет в JSON (`enabled: true`,
    `secret: false`, пустые массивы, тело «нет», `auth: null`, пустой `postman_extra` у строк) — иначе «снять и
    вернуть галочку» оставляло бы вкладку изменённой. `saved` и `draft` — копии нормализованного; «Наследовать»
    = `auth: null`. Ядро принимает такой JSON как есть. Ещё у вкладки: `save(api)` (`saveNodeContent` → `saved = draft`, дерево перечитывается, чтобы обновить метод),
    `revert()`.
  - `CodeEditor.svelte` — обёртка CodeMirror 6 (`value`, `language: "json" | "text"`, `readonly`, `onchange`);
    сам CodeMirror подгружается динамическим `import()` при первом показе.
  - `KeyValueTable.svelte` — таблица «включено / ключ / значение» поверх массива `Param`: правка на месте,
    пустая строка в конце для добавления, удаление строки; ключи `postman_extra` строк не трогает.
  - `RequestEditor.svelte` — `UrlBar` (выбор метода из списка GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS и
    свой метод текстом; поле URL; кнопка Send — пока неактивная заглушка, её подключит Task 6) и вкладки
    `Params` (query + отдельной группой Path-переменные), `Headers`, `Body`, `Auth` со счётчиками включённых строк.
  - `BodyEditor.svelte` — виды: «нет», «raw» (язык Text / JSON / XML / HTML / JavaScript, `CodeEditor`),
    «x-www-form-urlencoded» (`KeyValueTable`). Тело вида form-data, binary, GraphQL показывается только для
    чтения с пометкой «Редактирование появится позже»; переключение вида с такого тела на другой спрашивает
    подтверждение (данные будут заменены).
  - `AuthEditor.svelte` — тип: «Наследовать от родителя», «Без авторизации», Basic (логин, пароль), Bearer
    (токен), API Key (ключ, значение, «в заголовке / в query»), OAuth 2.0 (готовый токен, префикс, «в
    заголовке / в query»). Неподдержанный тип показывается названием и пометкой «не поддерживается», менять
    можно только на другой тип. Поля пароля и токена — замаскированы с кнопкой «показать».
  - `CollectionEditor.svelte` — вкладка коллекции: `Variables` (таблица «включено / имя / значение», значение
    секретной переменной замаскировано) и `Auth` (тот же `AuthEditor`, без варианта «Наследовать»). Отсутствие `auth` у
    коллекции показывается как «Без авторизации»; выбор «Без авторизации» ставит `auth: null`, а не `no_auth`;
    сохранённый `no_auth` показывается так же и не переписывается, пока тип не сменили.

**Поведение:**
- Вкладка с несохранёнными правками помечена точкой и `aria-label` «есть несохранённые изменения». Закрытие
  такой вкладки (крестик, Ctrl+W, средняя кнопка мыши) спрашивает: «Сохранить», «Не сохранять», «Отмена».
- Ctrl+S сохраняет активную вкладку; Ctrl+W закрывает; Ctrl+L — фокус в поле URL. Сбой сохранения —
  уведомление, черновик остаётся. Все сочетания приложения ловит один слушатель в `hotkeys.ts`:
  `window.addEventListener('keydown', handler, { capture: true })`; для своих сочетаний он вызывает
  `preventDefault()` и `stopPropagation()`, чтобы их не получили редактор кода и сам WebView2. Там же гасятся
  F5 и Ctrl+R **[моё решение]**: перезагрузка страницы в приложении стёрла бы все несохранённые черновики.
- Переключение вкладок — мгновенное, без анимации; содержимое неактивных вкладок не теряется.
- Изменение метода или любого поля сразу отражается в `draft`; до сохранения дерево показывает старый метод.
- Модель `Param`/`Variable` при правке сохраняет `postman_extra` элемента; новая строка создаётся с пустым
  `postman_extra` и `enabled: true`.

- [ ] **Step 1: Написать падающие тесты:**
  - `tabs.test.ts`: повторное открытие того же узла не создаёт вторую вкладку; правка `draft` делает `dirty`,
    возврат значения — снимает; снять и вернуть галочку у параметра без явного `enabled` — `dirty` снят; `save` вызывает `saveNodeContent` с черновиком и снимает `dirty`; сбой `save`
    оставляет `dirty`; `close` активирует соседнюю вкладку.
  - `RequestEditor.test.ts`: открытый запрос показывает метод и URL; правка URL ставит точку на вкладке;
    Ctrl+S вызывает `saveNodeContent` с новым URL и убирает точку; в `Params` снятие галочки меняет
    `enabled` и счётчик; добавление строки в `Headers` попадает в черновик; смена вида тела на raw JSON и ввод
    текста попадают в черновик (в тесте `CodeEditor` заменён заглушкой:
    `vi.mock('./CodeEditor.svelte', async () => ({ default: (await import('../lib/testing/CodeEditorStub.svelte')).default }))`); тело
    form-data показано с пометкой «появится позже»; выбор Bearer и ввод токена даёт
    `auth = { type: "bearer", token }` (имена — как сериализует `rqm-model`); «Наследовать» ставит `auth: null`;
    Ctrl+S и F5 дают `defaultPrevented === true`; закрытие вкладки с правками показывает три кнопки, «Отмена» оставляет вкладку.
  - `CollectionEditor.test.ts`: правка значения переменной и Ctrl+S → `saveNodeContent` с новым значением;
    `postman_extra` переменной в сохранённом содержимом тот же; секретное значение скрыто до нажатия «показать».
- [ ] **Step 2: `npm run test`** — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «Этап 1B: вкладки, редактор запроса и вкладка коллекции».

---

### Task 6: Send / Cancel и панель ответа

**Files:**
- Create: `ui/src/lib/sending.svelte.ts`, `ui/src/components/ResponsePanel.svelte`,
  `ui/src/components/ResponseStatus.svelte`, `ui/src/components/ResponseBody.svelte`,
  `ui/src/components/HeadersView.svelte`, `ui/src/components/CookiesView.svelte`,
  `ui/src/components/SendWarnings.svelte`
- Modify: `ui/src/components/UrlBar.svelte`, `ui/src/components/RequestEditor.svelte`, `ui/src/App.svelte`,
  `ui/src/lib/tabs.svelte.ts`, `ui/src/lib/hotkeys.ts`, `ui/src/lib/strings.ts`
- Test: `ui/src/lib/sending.test.ts`, `ui/src/components/ResponsePanel.test.ts`

**Interfaces:**
- Consumes: Task 3–5.
- Produces:
  - `sending.svelte.ts` — состояние отправки на вкладку: `state: "idle" | "sending"`, `result: SendResult | null`,
    `send(api, tab)` (создаёт `sendId`, вызывает `sendRequest(sendId, tab.nodeId, tab.draft)`), `cancel(api)`.
  - `ResponsePanel.svelte` — пустое состояние («Нажмите Send, чтобы получить ответ»), ожидание (индикатор
    появляется только через 150 мс), ошибка, ответ.

**Поведение:**
- Кнопка Send активна, когда URL черновика не пуст. Во время отправки она превращается в Cancel (переход
  `transform`/`opacity`, `--dur-3`); Ctrl+Enter отправляет активную вкладку, во время отправки — ничего не делает.
- Отправляется всегда черновик (`draft`), сохранять перед отправкой не нужно.
- `cancel(api)` сразу помечает текущую отправку отменённой: `state = "idle"`, `result` = ошибка вида
  `cancelled`, затем вызывает `cancelSend(sendId)`. Результат помеченной отправки, пришедший позже, отбрасывается,
  а если в нём есть ответ — его `body.id` освобождается.
- Ctrl+Enter добавляется в общий слушатель `hotkeys.ts` (с `preventDefault()`): иначе редактор кода сначала
  вставил бы в тело пустую строку.
- Результат принимается, только если его `sendId` — последний для этой вкладки; иначе результат отбрасывается,
  а его `body.id` сразу освобождается (`releaseBody`). При приходе нового ответа тело предыдущего освобождается;
  при закрытии вкладки — тоже.
- Строка статуса: код и фраза цветом по `statusClass` (и текстом), время (`formatMs(total_ms)`, во всплывающей
  подсказке — «до первого байта: …»), размер (`formatBytes(decoded_size)`, в подсказке — «по сети: …», если
  отличается). Если были редиректы — «редиректов: N», по щелчку — список переходов.
- Вкладки ответа: `Body`, `Headers` (счётчик), `Cookies` (счётчик). `Body`: переключатель Pretty / Raw —
  Pretty доступен и выбран по умолчанию только при `body.json`; содержимое запрашивается `responseBody(id,
  pretty)` и декодируется `TextDecoder("utf-8", { fatal: false })`; показывается в `CodeEditor` только для
  чтения (язык JSON в Pretty, иначе текст). `body.truncated` → полоса «Показаны первые 256 КБ из
  <formatBytes(total)>». `decode_error` → полоса «Не удалось распаковать ответ: показано как получено». Пустое
  тело — «Ответ без тела». Появление тела — без анимации.
- Ошибка отправки — текст по виду: `cancelled` — «Запрос отменён» (не как ошибка, обычным цветом); `timeout` —
  «Сервер не ответил вовремя»; `connect` — «Не удалось подключиться»; `tls` — «Ошибка сертификата»;
  `invalid_url` — «Неверный адрес»; `too_many_redirects` — «Слишком много редиректов»; `unconfirmed_files` —
  «Файлы нужно выбрать заново: <поля>»; `io`, `other` — «Ошибка отправки». Под текстом — `message` ядра
  моноширинным шрифтом.
- Предупреждения (`SendWarnings`) — над ответом и над ошибкой: `scripts_not_executed` — заметная полоса «Скрипты
  не выполнены: <узлы>. Выполнение скриптов появится на следующем этапе»; `unresolved_variables` —
  «Не найдены переменные: <имена>»; `body_pruned` — «Тело не отправлено: метод <метод> его не передаёт»;
  `unsupported_auth` — «Авторизация <тип> не поддерживается, запрос отправлен без неё».

- [ ] **Step 1: Написать падающие тесты:**
  - `sending.test.ts`: `send` передаёт черновик, а не сохранённое содержимое; пока запрос идёт — `state =
    "sending"`; `cancel` вызывает `cancelSend` с тем же `sendId` и сразу даёт `state = "idle"` с
    результатом `cancelled`; если после `cancel` приходит результат с ответом — `result` остаётся `cancelled` и
    вызван `releaseBody`; два `send` подряд — результат первого
    отброшен и для его `body.id` вызван `releaseBody`, в `result` — второй; новый ответ освобождает тело
    предыдущего; ошибка самой команды (`sendRequest` отклонён) → `state = "idle"` и уведомление.
  - `ResponsePanel.test.ts` (fake api; `CodeEditor` — заглушка): до отправки — пустое состояние; Ctrl+Enter
    вызывает отправку и даёт `defaultPrevented === true`; во время ожидания кнопка называется Cancel; ответ `200` с `json: true` — статус с классом
    `ok`, время и размер отформатированы, запрошено `responseBody(id, true)`, переключение на Raw запрашивает
    `responseBody(id, false)`; ответ с `json: false` — Pretty недоступен; `truncated` — видна полоса с размером;
    байты не-UTF-8 не вызывают исключения; `Headers` и `Cookies` показывают строки и счётчики; результат
    `cancelled` — текст «Запрос отменён» без класса ошибки; `connect` — текст и `message`; предупреждение
    `scripts_not_executed` видно и при ответе, и при ошибке; пустой URL — Send неактивен.
- [ ] **Step 2: `npm run test`** — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «Этап 1B: отправка, отмена и панель ответа».

---

### Task 7: Сборка приложения, список проверки для владельца, документы

**Files:**
- Create: `stage1b-results.md` — что сделано, числа тестов, все
  **[моё решение]** этого плана и как реализованы, что отложено в части 3, размеры exe и установщика
- Create: `stage1b-owner-checklist.md` — список проверки окна (ниже)
- Modify: `docs/design.md` — раздел 12 (1B выполнен, перечень
  отложенного в части 3), раздел 4 (имена команд — ссылка на «Контракт команд» этого плана)
- Modify: `spike/README.md` — пробник окна больше не образец для продукта, остаётся ради замеров до части 3

- [ ] **Step 1: Полный прогон:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`;
  в `ui/`: `npm run check`, `npm run test`, `npm run build`.
- [ ] **Step 2: Собрать приложение:** в `ui/` — `npm run tauri build`. По выводу убедиться, что взят
  `src-tauri/tauri.conf.json`, а не `spike/shell`. Если CLI проект не нашёл — в PowerShell из `ui/`:
  `$env:TAURI_APP_PATH='../src-tauri'; npx tauri build`. Рабочую команду записать в документ результатов. Ожидается
  `target/release/requestmaster.exe` и установщик NSIS в `target/release/bundle/nsis/`. Приложение не запускать.
  Размеры файлов записать; установщик должен быть ≤ 20 МБ (спека, раздел 10).
- [ ] **Step 3: Проверить, что ядро не зависит от Tauri:** `cargo tree -p rqm-runner -i tauri` и то же для
  остальных `rqm-*` — пакета нет. Проверить `git ls-files spike/samples` — пусто.
- [ ] **Step 4: Написать список проверки для владельца** (простыми словами, с путём к exe и шагами):
  запуск; импорт своей коллекции кнопкой и перетаскиванием; отчёт импорта; дерево, сворачивание, фильтр,
  клавиатура; открыть запрос, поменять URL, точка на вкладке, Ctrl+S; вкладка коллекции — вписать значение
  переменной (например токен) и сохранить; Send обычного запроса — статус, время, размер, Pretty/Raw, Headers,
  Cookies; запрос со скриптом — полоса «Скрипты не выполнены»; Cancel на долгом запросе; запрос на
  несуществующий адрес — понятная ошибка; закрыть вкладку с правками — вопрос; перезапуск — коллекция на месте;
  что бросилось в глаза по виду и скорости. К каждому шагу — что должно получиться.
- [ ] **Step 5: Написать документ результатов и обновить спеку.**
- [ ] **Step 6: Commit** — «Этап 1B: сборка приложения, список проверки и итоги».
