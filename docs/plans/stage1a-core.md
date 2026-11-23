# RequestMaster, этап 1A «Ядро» — план реализации

**Goal:** Ядро RequestMaster на Rust без окна: импорт и экспорт Postman, хранилище, переменные, отправка
HTTP-запроса — всё проверяется автотестами, включая настоящую коллекцию владельца.

**Architecture:** Cargo workspace в корне репозитория, шесть crates `rqm-*` (без `rqm-script` — это этап 2).
Зависимости в одну сторону: `rqm-runner` → (`rqm-http`, `rqm-vars`, `rqm-store`) → `rqm-model`;
`rqm-postman` → `rqm-model`. Ни один crate не зависит от Tauri. Окно (Tauri + Svelte) — следующий план, 1B.

**Tech Stack:** Rust (edition 2021), `serde`/`serde_json` (`preserve_order`), `rusqlite` (bundled),
`reqwest` 0.13 + `rustls`/`ring`, `tokio`, `tokio-util` (`CancellationToken`), `wiremock` (тесты).

**Spec:** `docs/design.md` (дальше «спека»);
итоги этапа 0 — `stage0-findings.md` (дальше «итоги»).

Этап 1 спеки разбит на три плана **[моё решение]**: **1A «Ядро»** (этот), **1B «Окно: основа»** (команды Tauri,
стиль, дерево, вкладки, редактор запроса, отправка, ответ, импорт), **1C «Окно: полнота»** (редакторы
коллекции/папки/окружений, экспорт, перетаскивание, подсветка переменных, анимации). После 1A запускать
глазами нечего — результат виден по тестам; первое рабочее окно даёт 1B.

Явно отложено в 1B: события прогресса запроса (в 1B — начало и конец на стороне команды Tauri, trait `HttpEngine`
не меняется) и замеры целей раздела 10 спеки (накладные расходы на запрос, запуск, переключение вкладок).

## Global Constraints

- Ветка: `feature/stage1a-core`, создаётся от `feature/stage0-spike`. В `main`/`master` не коммитить. Push — никогда.
- Сообщения коммитов на русском.
- Приложения и dev-серверы не запускать. `cargo build`, `cargo test` — можно.
- Сеть: только crates.io. Тесты ходят только на `127.0.0.1` (локальный тестовый сервер).
- Версии зависимостей закреплять точно (`=x.y.z`). Что уже закреплено в `spike/*/Cargo.toml` — брать те же
  версии: `serde =1.0.229`, `serde_json =1.0.154` (`preserve_order`), `reqwest =0.13.5`
  (`default-features = false`, features `rustls-no-provider`, `stream`, `multipart`, `system-proxy`),
  `rustls =0.23.46` (`ring`, `std`, `tls12`, `logging`), `hyper-util =0.1.21` (`client-proxy-system`),
  `http =1.5.0`, `flate2 =1.1.11`, `brotli =9.1.1`, `zstd =0.14.3`, `tokio =1.54.1`, `tokio-util =0.7.20`,
  `wiremock =0.6.5`, `rcgen =0.14.10`, `tokio-rustls =0.26.6`; `rusqlite` — версия и features из
  `spike/shell/src-tauri/Cargo.toml`. Новые crates (`uuid`, `base64`, `url`, `thiserror`, `tempfile`,
  `cookie_store` или аналог) — последняя стабильная версия, закреплённая точно.
- В дереве зависимостей не должно быть `aws-lc-rs`, `aws-lc-sys`, `native-tls`, `openssl` (проверка — Task 6).
- HTTP/2 не включать (feature `http2` у `reqwest` выключена) **[моё решение]**. PAC/WPAD для прокси в первой
  версии не поддерживается **[моё решение]** (итоги §7 п. 6).
- Все публичные типы — `Debug, Clone`; enum-ы ошибок — `Debug` + `thiserror::Error` (без `Clone`, если мешает
  источник ошибки); типы, которые тесты сравнивают через `==`, — ещё `PartialEq`.
- Данные владельца: `spike/samples/` в git не попадает. Значения из образцов (URL, хосты, имена, токены, тела)
  нельзя печатать в вывод тестов, сообщения `assert!`, отчёты и коммиты — только счётчики. Пути JSON из образцов
  тоже не печатать (в них попадают имена, заданные владельцем) — только число расхождений. Имя файла образца
  в отчёты не писать. Тест на образце при пустой папке `spike/samples/` печатает `owner samples: 0 (skipped)`
  и проходит; файл неподдержанного формата считается в `skipped` и тест не роняет.
- Схема БД — ровно DDL из Task 5, согласованный владельцем вместе с этим планом. Любая другая таблица, колонка
  или индекс — остановиться и спросить.
- Только то, что в задаче: без CLI, без `rqm-script`, без истории запросов, без настроек «на будущее».
- Публичные типы и функции — с doc-комментарием на английском (зачем и откуда значение). `unwrap()`/`expect()`
  вне тестов — только там, где сбой невозможен, с комментарием почему.
- Каждая задача заканчивается зелёными `cargo test -p <crate>` и `cargo clippy -p <crate> -- -D warnings`.
- Пометки **[моё решение]** и **[проверить]** — как в спеке. Всё с **[проверить]** исполнитель реализует как
  написано и перечисляет в отчёте задачи.

### Правила сохранения данных Postman (общие для Task 1–3)

- **P1.** У каждого узла модели есть `postman_extra` — исходный JSON-объект этого узла из Postman без ключей
  `item` (потомки) и `name`. Для запроса это объект элемента целиком вместе с вложенным `request`.
  У узла, созданного в приложении, `postman_extra` пуст.
- **P2.** Экспорт узла начинается с копии `postman_extra`. Типизированное поле (`url`, `header`, `body`, `auth`,
  `event`, `variable`, `protocolProfileBehavior`, `method`, `values`) записывается заново **только если оно
  изменилось**: если разбор исходного значения даёт то же, что лежит в модели, остаётся исходное значение
  дословно. Так явный `null`, порядок ключей и неизвестные вложенные ключи нетронутого поля сохраняются сами.
- **P3.** У элементов списков (`Param`, `Variable`, `FormPart`) свой `postman_extra` — исходный JSON-объект
  элемента целиком (`key`, `value`, `description`, `type`, `disabled`/`enabled` и всё остальное). Когда список
  изменён и генерируется заново, каждый элемент строится от своего `postman_extra`, и типизированный ключ
  перезаписывается только если изменился: `value` — если строковая форма исходного значения не равна значению
  модели (так сохраняются числа, булевы и `null`); `disabled`/`enabled` — если изменился признак; `type`
  переменной — `"secret"` при `secret = true`; `"default"`, если был `"secret"`, а стало `false`; иначе исходный,
  а если его не было — `"default"` у окружения/globals и `"string"` у переменной коллекции. У элемента,
  созданного в приложении, `postman_extra` пуст.
- **P4.** Скрипт в модели — одна строка (`exec`, склеенный через `\n`). При изменении скрипта в исходном
  событии заменяется только `script.exec` (строка режется по `\n`), остальные ключи события сохраняются.

Это уточнение механизма раздела 5 спеки **[моё решение]**: результат тот же («неизвестное не теряется»),
но вместо «мешка» на каждом уровне зеркальных структур хранится исходный фрагмент целиком.

## Review Focus

1. **Отредактированный запрос теряет чужие поля при экспорте.** Изменили одно значение заголовка → у остальных
   заголовков и у запроса должны остаться `description`, ID и неизвестные ключи (тест в Task 3).
2. **Редирект на другой сайт уносит секреты.** `Authorization` и `Cookie` не должны уйти на другой origin;
   после смены метода на GET не должны повторяться заголовки тела (тесты в Task 7).
3. **Переменная ссылается сама на себя или по кругу** (`a = {{a}}`, `a = {{b}}`, `b = {{a}}`) — подстановка
   обязана завершиться, а не зависнуть (тест в Task 4).
4. **Пустое или сжатое-пустое тело** (`HEAD`, `204`, `304` с `Content-Encoding: gzip`) — ответ без ошибки
   распаковки (тест в Task 6).
5. **Перемещение папки в саму себя или в своего потомка** — отказ с ошибкой, дерево не портится (тест в Task 5).

---

### Task 1: Workspace и `rqm-model`

**Files:**
- Create: `Cargo.toml` (workspace: `members = ["crates/*"]`, `resolver = "2"`, `[workspace.dependencies]`
  с закреплёнными версиями, `[profile.release]` как в `spike/shell/src-tauri/Cargo.toml`)
- Create: `crates/rqm-model/Cargo.toml`, `crates/rqm-model/src/lib.rs`, `src/node.rs`, `src/request.rs`,
  `src/environment.rs`
- Test: `crates/rqm-model/tests/model.rs`
- Modify: `.gitignore` — ничего не добавлять, если `target/` уже покрывает корень (проверить `git status`)

**Interfaces:**
- Consumes: ничего.
- Produces (все типы `Debug, Clone, PartialEq`; структуры содержимого и их части — ещё `Serialize, Deserialize`;
  enum-ы сериализуются в `snake_case`. `Node` и `NodeContent` не реализуют `Deserialize`: вид хранится в
  `Node.kind`, содержимое пишется как голая структура, читается только через `NodeContent::from_json(kind, json)`):

```rust
pub type NodeId = String;                       // UUID v4, lower-case hyphenated
pub type Extra = serde_json::Map<String, serde_json::Value>;
pub fn new_id() -> NodeId;

pub enum NodeKind { Collection, Folder, Request }
pub struct Node { pub id: NodeId, pub parent_id: Option<NodeId>, pub kind: NodeKind,
                  pub name: String, pub sort_order: i64, pub content: NodeContent }
pub enum NodeContent { Collection(CollectionContent), Folder(FolderContent), Request(RequestContent) }
impl NodeContent { pub fn to_json(&self) -> String;
                   pub fn from_json(kind: NodeKind, json: &str) -> Result<Self, serde_json::Error>; }

pub struct CollectionContent { pub auth: Option<Auth>, pub variables: Vec<Variable>,
                               pub scripts: Scripts, pub postman_extra: Extra }
pub struct FolderContent { pub auth: Option<Auth>, pub scripts: Scripts, pub postman_extra: Extra }
pub struct RequestContent { pub method: String, pub url: String, pub query: Vec<Param>,
    pub path_variables: Vec<Param>, pub headers: Vec<Param>, pub body: Body, pub auth: Option<Auth>,
    pub scripts: Scripts, pub settings: RequestSettings,
    /// Lower-case names of automatic headers switched off for this request (Headers tab).
    pub disabled_auto_headers: Vec<String>, pub postman_extra: Extra }

pub struct Param { pub key: String, pub value: String, pub enabled: bool, pub postman_extra: Extra }
pub struct Variable { pub key: String, pub value: String, pub secret: bool, pub enabled: bool,
                      pub postman_extra: Extra }
pub struct Scripts { pub pre_request: String, pub test: String }      // Default: both empty
impl Scripts { pub fn is_empty(&self) -> bool; }                      // true when both are blank after trim

pub enum Auth { NoAuth, Basic { username: String, password: String }, Bearer { token: String },
    ApiKey { key: String, value: String, placement: ApiKeyPlacement },
    /// `header_prefix` is already trimmed; empty = send the bare token.
    OAuth2Token { token: String, header_prefix: String, placement: ApiKeyPlacement },
    Unsupported { auth_type: String } }
pub enum ApiKeyPlacement { Header, Query }

pub enum Body { None, Raw { text: String, language: RawLanguage }, UrlEncoded(Vec<Param>),
    FormData(Vec<FormPart>), Binary { file: Option<FileRef> },
    GraphQl { query: String, variables: String } }
pub enum RawLanguage { Text, Json, Xml, Html, JavaScript }
pub struct FormPart { pub key: String, pub value: FormValue, pub enabled: bool,
                      pub content_type: Option<String>, pub postman_extra: Extra }
pub enum FormValue { Text(String), Files(Vec<FileRef>) }
pub struct FileRef { pub path: String, pub confirmed: bool }

/// `None` = not set on the request, the application-wide setting applies.
pub struct RequestSettings { pub follow_redirects: Option<bool>, pub max_redirects: Option<u32>,
    pub strict_ssl: Option<bool>, pub disable_body_pruning: Option<bool> }   // Default: all None

pub enum EnvKind { Environment, Globals }
pub struct Environment { pub id: String, pub kind: EnvKind, pub name: String,
                         pub variables: Vec<Variable>, pub postman_extra: Extra }
pub const GLOBALS_ID: &str = "globals";
impl Environment {
    pub fn empty_globals() -> Self;                  // id = GLOBALS_ID, kind = Globals, name = "Globals", no variables
    /// JSON for the store: `{"variables": [...], "postman_extra": {...}}` (empty parts are skipped).
    pub fn content_json(&self) -> String;
    /// `kind` is `Globals` when `id == GLOBALS_ID`, otherwise `Environment`.
    pub fn from_parts(id: &str, name: &str, content_json: &str) -> Result<Self, serde_json::Error>;
}
```

  `url` — адрес без query-части; query живёт в `query` (включая отключённые параметры). Фрагмент `#…`, если он
  был, остаётся в конце `url`; при сборке адреса query вставляется перед `#`.
  `Auth = None` у узла значит «наследовать от родителя»; `Some(Auth::NoAuth)` — «явно без авторизации».
  Пустые `postman_extra` и значения по умолчанию при сериализации пропускаются (`skip_serializing_if`),
  при чтении восстанавливаются (`#[serde(default)]`).

- [ ] **Step 1: Создать workspace и пустой crate; `cargo build -p rqm-model`** — ожидается успех.

- [ ] **Step 2: Написать падающие тесты `crates/rqm-model/tests/model.rs`**

```rust
use rqm_model::*;

fn sample_request() -> RequestContent {
    RequestContent {
        method: "POST".into(), url: "https://example.test/a/:id".into(),
        query: vec![Param { key: "q".into(), value: "1".into(), enabled: false, postman_extra: Extra::new() }],
        path_variables: vec![Param { key: "id".into(), value: "7".into(), enabled: true, postman_extra: Extra::new() }],
        headers: vec![], body: Body::Raw { text: "{}".into(), language: RawLanguage::Json },
        auth: Some(Auth::NoAuth), scripts: Scripts::default(), settings: RequestSettings::default(),
        disabled_auto_headers: vec![], postman_extra: Extra::new(),
    }
}

#[test] fn request_content_json_round_trip() {
    let c = NodeContent::Request(sample_request());
    let back = NodeContent::from_json(NodeKind::Request, &c.to_json()).unwrap();
    assert_eq!(c, back);
}
#[test] fn defaults_are_not_written() {
    let json = NodeContent::Folder(FolderContent { auth: None, scripts: Scripts::default(), postman_extra: Extra::new() }).to_json();
    assert_eq!(json, "{}");
}
#[test] fn inherit_and_noauth_are_different() {
    let mut r = sample_request();
    let a = NodeContent::Request(r.clone()).to_json();
    r.auth = None;
    let b = NodeContent::Request(r).to_json();
    assert_ne!(a, b);
}
#[test] fn wrong_kind_is_an_error_not_a_panic() {
    let c = NodeContent::Request(sample_request()).to_json();
    let _ = NodeContent::from_json(NodeKind::Collection, &c);            // must not panic, either result is fine
    assert!(NodeContent::from_json(NodeKind::Request, "not json").is_err());
}
#[test] fn new_id_is_uuid_v4() {
    let id = new_id();
    assert_eq!(id.len(), 36);
    assert_eq!(id.as_bytes()[14], b'4');
    assert_ne!(id, new_id());
}
#[test] fn scripts_is_empty_ignores_whitespace() {
    assert!(Scripts { pre_request: " \n".into(), test: String::new() }.is_empty());
    assert!(!Scripts { pre_request: "x".into(), test: String::new() }.is_empty());
}
#[test] fn empty_globals_has_fixed_id() {
    let g = Environment::empty_globals();
    assert_eq!((g.id.as_str(), g.kind), (GLOBALS_ID, EnvKind::Globals));
}
#[test] fn environment_content_round_trip() {
    let mut e = Environment { id: new_id(), kind: EnvKind::Environment, name: "E".into(), postman_extra: Extra::new(),
        variables: vec![Variable { key: "k".into(), value: "v".into(), secret: true, enabled: false, postman_extra: Extra::new() }] };
    e.postman_extra.insert("id".into(), serde_json::json!("x"));
    assert_eq!(Environment::from_parts(&e.id, &e.name, &e.content_json()).unwrap(), e);
    assert_eq!(Environment::from_parts(GLOBALS_ID, "Globals", "{}").unwrap(), Environment::empty_globals());
    assert!(Environment::from_parts("x", "y", "nope").is_err());
}
```

- [ ] **Step 3: `cargo test -p rqm-model`** — ожидается ошибка компиляции (типов нет).
- [ ] **Step 4: Реализовать типы по блоку Interfaces.**
- [ ] **Step 5: `cargo test -p rqm-model` и `cargo clippy -p rqm-model -- -D warnings`** — 8 passed, без предупреждений.
- [ ] **Step 6: Commit** — `git add Cargo.toml Cargo.lock crates/rqm-model` → «Этап 1A: workspace и доменная модель rqm-model».

---

### Task 2: `rqm-postman` — импорт коллекции и окружений в модель

**Files:**
- Create: `crates/rqm-postman/Cargo.toml`, `src/lib.rs`, `src/detect.rs`, `src/import.rs`, `src/report.rs`,
  `src/diff.rs`
- Create: `crates/rqm-postman/tests/fixtures/collection_small.json`, `environment.json`, `globals.json`
  (синтетика, написать вручную по описанию ниже; данных владельца в fixtures быть не должно)
- Test: `crates/rqm-postman/tests/import.rs`

**Interfaces:**
- Consumes: всё из Task 1.
- Produces:

```rust
pub enum Detected { Collection, Environment, Globals }
/// `NotJson` carries only the parser message (kind, line, column), never a fragment of the input;
/// `UnsupportedSchema` carries only the version.
pub enum ImportError { NotJson(String), UnknownFormat, UnsupportedSchema(String) }   // thiserror, Display in English
/// Strips a UTF-8 BOM, then parses.
pub fn parse_text(text: &str) -> Result<serde_json::Value, ImportError>;
pub fn detect(json: &serde_json::Value) -> Result<Detected, ImportError>;

pub struct ImportedCollection { pub nodes: Vec<Node>, pub report: ImportReport }
/// `nodes[0]` is the collection; every parent precedes its children; siblings keep file order
/// with `sort_order` 0, 1, 2…; all ids are fresh (`new_id()`).
pub fn import_collection(json: &serde_json::Value) -> Result<ImportedCollection, ImportError>;
/// Accepts both environment and globals files. Globals get `id = GLOBALS_ID`, environments a fresh id.
pub fn import_environment(json: &serde_json::Value) -> Result<Environment, ImportError>;

pub struct ImportReport { pub items: Vec<ReportItem> }
pub struct ReportItem { pub kind: ReportKind, pub node_id: NodeId, pub node_path: String, pub detail: String }
pub enum ReportKind { UnsupportedAuth, SettingNotApplied, HasScripts, FileReference }

/// Empty result = semantically equal. Object key order is ignored, array order is not.
pub fn semantic_diff(a: &serde_json::Value, b: &serde_json::Value) -> Vec<String>;   // JSON paths only, no values
```

**Правила разбора** (где Postman допускает две формы — принимаются обе):
- `detect`: `info.schema` содержит `/v2.1.0/` → `Collection`; содержит `/v2.0.0/` → `UnsupportedSchema("v2.0.0")`;
  `_postman_variable_scope` = `"environment"` / `"globals"` → соответствующий вид; иначе `UnknownFormat`.
- Имя коллекции — `info.name`; `postman_extra` коллекции — исходный объект без `item` (то есть `info`,
  `variable`, `auth`, `event`, `protocolProfileBehavior` и всё неизвестное). Правило P1.
- Элемент с массивом `item` — папка, иначе — запрос. `request` строкой = `GET` на этот URL.
- `url` строкой или объектом. Адрес — `raw`; если `raw` нет или он не строка — собрать из частей:
  `protocol://` + `host` (массив через `.` или строка) + `:port` + `/` + `path` (массив через `/` или строка).
  Модельный `url` — адрес без query: часть от `?` до `#` (или до конца) вырезается, фрагмент `#…` остаётся.
  `query` — из `url.query`, а если его нет — разбором вырезанной части по `&` и первому `=` (без
  percent-декодирования). `disabled: true` → `enabled: false`. Значение `null` → `""`. `path_variables` — из
  `url.variable`.
- `header` массивом или строкой (`"A: 1\nB: 2"`); `disabled` как выше.
- `body.mode`: `raw` (язык из `options.raw.language`: `json`/`xml`/`html`/`javascript`, иначе `Text`),
  `urlencoded`, `formdata` (`type: "file"` → `FormValue::Files`; `src` строкой, массивом строк или `null` →
  ноль или больше `FileRef { confirmed: false }`), `file` (`file.src`), `graphql` (`query`; `variables` строкой, а любое другое JSON-значение — компактной
  JSON-строкой; наличие `operationName` → запись `SettingNotApplied` с `detail = "body.graphql.operationName"`).
  Нет `body` или `body.disabled: true` без содержимого → `Body::None`. Неизвестный `mode` → `Body::None`
  + запись `SettingNotApplied` с `detail = "body.mode: <mode>"`.
- `auth`: нет ключа или `null` → `None`; `noauth` → `NoAuth`; `basic` (`username`, `password`); `bearer` (`token`);
  `apikey` (`key`, `value`, `in` = `header` | `query`, по умолчанию `header`); `oauth2` → `OAuth2Token`:
  `token` = `accessToken`; `header_prefix` — если ключа `headerPrefix` нет или он `null`, то `"Bearer"`, иначе
  строка с обрезанными пробелами (пустая допустима); `placement` = `Query` при `addTokenTo == "queryParams"`,
  иначе `Header`; `oauth2` с `tokenType` = `mac` (без учёта регистра) → `Unsupported`; любой другой тип → `Unsupported { auth_type }`
  + запись `UnsupportedAuth`. Параметры auth приходят массивом `{key, value, type}` или объектом — принять обе формы.
- `event`: `listen: "prerequest"` → `scripts.pre_request`, `"test"` → `scripts.test`; `script.exec` массивом
  строк (склеить через `\n`) или строкой. Непустой скрипт на любом узле → запись `HasScripts`.
- `variable` коллекции: `{key, value, type, disabled}`; значение не-строка (число, булево, `null`) → строковая
  форма (`null` → `""`); `secret = (type == "secret")`.
- Окружение/globals: `values: [{key, value, type, enabled}]`, `enabled` по умолчанию `true`; значение не-строка →
  строковая форма (`null` → `""`); `secret = (type == "secret")`; `postman_extra` — исходный объект без `name`
  (массив `values` остаётся внутри — правило P1/P2).
- `protocolProfileBehavior`: `followRedirects`, `maxRedirects`, `strictSSL`, `disableBodyPruning` →
  `RequestSettings`; `disabledSystemHeaders` (объект `{имя: true}`) → `disabled_auto_headers` (имена в нижнем
  регистре, только со значением `true`); каждый другой ключ → запись `SettingNotApplied` с `detail = <имя ключа>`.
  На папке и коллекции все ключи → `SettingNotApplied` (настройки папок в первой версии не применяются).
- Файловая ссылка (`FormValue::Files` с путём или `Body::Binary { file: Some(..) }`) → запись `FileReference`,
  `detail` — имя поля формы (`key`) или `"binary"`. Путь к файлу в отчёт не пишется.
- `node_path` в отчёте — имена от коллекции до узла через ` / `.
- Неизвестные ключи и типы нигде не роняют импорт.

`disableBodyPruning` поднят в настройки запроса **[моё решение]** (в спеке — «сохраняется, но не применяется»;
используется в 2 запросах образца владельца, итоги §7 п. 2). Ждёт подтверждения владельца вместе с планом.

**Fixture `collection_small.json`** (v2.1.0) должен содержать: коллекцию с `auth` типа `bearer`, двумя
переменными (одна `disabled`), pre-request скриптом и неизвестным ключом `_exporter_id`; папку `F1` с вложенной
папкой `F2`; в `F2` — запрос `R1` (`POST`, `url` объектом с `query` (один `disabled`) и `variable`, заголовки
массивом (один `disabled`, у одного `description`), `raw` JSON, test-скрипт массивом строк,
`protocolProfileBehavior: {"disableBodyPruning": true, "followRedirects": false, "insecureHTTPParser": true}`);
в `F1` — запрос `R2` (`request` строкой); в корне — `R3` (`auth: null`, `formdata` с текстовым полем и файлом,
`header` строкой), `R4` (`auth` типа `awsv4`, `body.mode: "file"`), `R5` (`auth: {"type": "noauth"}`,
`urlencoded`), `R6` (`graphql`, `auth` типа `apikey` с `in: "query"`).

- [ ] **Step 1: Написать fixtures и падающие тесты `tests/import.rs`**

```rust
use rqm_model::*;
use rqm_postman::*;

fn load(name: &str) -> serde_json::Value {
    parse_text(&std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap()).unwrap()
}
fn find<'a>(nodes: &'a [Node], name: &str) -> &'a Node { nodes.iter().find(|n| n.name == name).unwrap() }
fn req(n: &Node) -> &RequestContent { match &n.content { NodeContent::Request(r) => r, _ => panic!("not a request") } }

#[test] fn detects_formats() {
    assert!(matches!(detect(&load("collection_small.json")), Ok(Detected::Collection)));
    assert!(matches!(detect(&load("environment.json")), Ok(Detected::Environment)));
    assert!(matches!(detect(&load("globals.json")), Ok(Detected::Globals)));
    assert!(matches!(detect(&serde_json::json!({"info": {"schema": "https://schema.getpostman.com/json/collection/v2.0.0/collection.json"}})), Err(ImportError::UnsupportedSchema(_))));
    assert!(matches!(detect(&serde_json::json!([1, 2])), Err(ImportError::UnknownFormat)));
}
#[test] fn bom_is_stripped() { assert!(parse_text("\u{feff}{\"a\":1}").is_ok()); }
#[test] fn not_json_is_an_error() { assert!(matches!(parse_text("<html>"), Err(ImportError::NotJson(_)))); }

#[test] fn tree_shape_and_order() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    assert_eq!(c.nodes[0].kind, NodeKind::Collection);
    assert_eq!(c.nodes.iter().filter(|n| n.kind == NodeKind::Folder).count(), 2);
    assert_eq!(c.nodes.iter().filter(|n| n.kind == NodeKind::Request).count(), 6);
    let root = &c.nodes[0].id;
    let top: Vec<_> = c.nodes.iter().filter(|n| n.parent_id.as_ref() == Some(root)).map(|n| (n.name.as_str(), n.sort_order)).collect();
    assert_eq!(top, vec![("F1", 0), ("R3", 1), ("R4", 2), ("R5", 3), ("R6", 4)]);
    assert_eq!(find(&c.nodes, "F2").parent_id.as_ref(), Some(&find(&c.nodes, "F1").id));
    // every parent precedes its children
    for (i, n) in c.nodes.iter().enumerate().skip(1) {
        let p = n.parent_id.as_ref().unwrap();
        assert!(c.nodes[..i].iter().any(|x| &x.id == p));
    }
}
#[test] fn request_r1_fields() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    let r = req(find(&c.nodes, "R1"));
    assert_eq!(r.method, "POST");
    assert!(!r.url.contains('?'));
    assert_eq!(r.query.iter().filter(|p| !p.enabled).count(), 1);
    assert_eq!(r.path_variables.len(), 1);
    assert_eq!(r.headers.iter().filter(|p| !p.enabled).count(), 1);
    assert!(r.headers.iter().any(|p| p.postman_extra.contains_key("description")));
    assert!(matches!(&r.body, Body::Raw { language: RawLanguage::Json, .. }));
    assert!(r.scripts.test.contains('\n'));
    assert_eq!(r.settings, RequestSettings { follow_redirects: Some(false), max_redirects: None, strict_ssl: None, disable_body_pruning: Some(true) });
    assert_eq!(r.auth, None);
}
#[test] fn auth_variants() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    assert!(matches!(&c.nodes[0].content, NodeContent::Collection(x) if matches!(x.auth, Some(Auth::Bearer { .. }))));
    assert_eq!(req(find(&c.nodes, "R3")).auth, None);
    assert!(matches!(req(find(&c.nodes, "R4")).auth, Some(Auth::Unsupported { ref auth_type }) if auth_type == "awsv4"));
    assert_eq!(req(find(&c.nodes, "R5")).auth, Some(Auth::NoAuth));
    assert!(matches!(req(find(&c.nodes, "R6")).auth, Some(Auth::ApiKey { placement: ApiKeyPlacement::Query, .. })));
}
#[test] fn bodies_and_files() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    assert_eq!(req(find(&c.nodes, "R2")).method, "GET");
    match &req(find(&c.nodes, "R3")).body {
        Body::FormData(parts) => {
            assert!(parts.iter().any(|p| matches!(p.value, FormValue::Text(_))));
            assert!(parts.iter().any(|p| matches!(&p.value, FormValue::Files(f) if f.len() == 1 && !f[0].confirmed)));
        }
        _ => panic!("expected form-data"),
    }
    assert_eq!(req(find(&c.nodes, "R3")).headers.len(), 2);
    assert!(matches!(&req(find(&c.nodes, "R4")).body, Body::Binary { file: Some(f) } if !f.confirmed));
    assert!(matches!(&req(find(&c.nodes, "R5")).body, Body::UrlEncoded(_)));
    assert!(matches!(&req(find(&c.nodes, "R6")).body, Body::GraphQl { .. }));
}
#[test] fn collection_variables_and_extra() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    let NodeContent::Collection(col) = &c.nodes[0].content else { panic!() };
    assert_eq!(col.variables.len(), 2);
    assert_eq!(col.variables.iter().filter(|v| !v.enabled).count(), 1);
    assert!(!col.scripts.pre_request.is_empty());
    assert!(col.postman_extra.contains_key("_exporter_id"));
    assert!(col.postman_extra.contains_key("info"));
    assert!(!col.postman_extra.contains_key("item"));
}
#[test] fn report_lists_everything_unsupported() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    let count = |k: ReportKind| c.report.items.iter().filter(|i| std::mem::discriminant(&i.kind) == std::mem::discriminant(&k)).count();
    assert_eq!(count(ReportKind::UnsupportedAuth), 1);
    assert_eq!(count(ReportKind::SettingNotApplied), 1);          // insecureHTTPParser
    assert_eq!(count(ReportKind::HasScripts), 2);                 // collection + R1
    assert_eq!(c.report.items.len(), 6);
    assert_eq!(count(ReportKind::FileReference), 2);              // R3 file part + R4 binary
    let item = c.report.items.iter().find(|i| matches!(i.kind, ReportKind::SettingNotApplied)).unwrap();
    assert_eq!(item.detail, "insecureHTTPParser");
    assert!(item.node_path.ends_with("F1 / F2 / R1"));
}
#[test] fn environment_and_globals() {
    let e = import_environment(&load("environment.json")).unwrap();
    assert_eq!(e.kind, EnvKind::Environment);
    assert!(e.variables.iter().any(|v| v.secret));
    assert!(e.variables.iter().any(|v| !v.enabled));
    assert!(e.postman_extra.contains_key("values") && !e.postman_extra.contains_key("name"));
    assert_eq!(e.variables.iter().find(|v| v.key == "num").unwrap().value, "5");
    assert_eq!(e.variables.iter().find(|v| v.key == "nil").unwrap().value, "");
    let g = import_environment(&load("globals.json")).unwrap();
    assert_eq!((g.id.as_str(), g.kind), (GLOBALS_ID, EnvKind::Globals));
}
#[test] fn unknown_shapes_do_not_break_import() {
    let j = serde_json::json!({"info": {"name": "x", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
        "item": [{"name": "odd", "request": {"method": 5, "url": {"raw": 7}, "header": null, "body": {"mode": "weird"}, "auth": {"type": 3}}, "event": "nope"}]});
    let c = import_collection(&j).unwrap();
    assert_eq!(c.nodes.len(), 2);
}
#[test] fn semantic_diff_ignores_key_order_only() {
    let a = serde_json::json!({"a": 1, "b": [1, 2]});
    assert!(semantic_diff(&a, &serde_json::json!({"b": [1, 2], "a": 1})).is_empty());
    assert_eq!(semantic_diff(&a, &serde_json::json!({"a": 1, "b": [2, 1]})).len(), 2);
    assert_eq!(semantic_diff(&a, &serde_json::json!({"a": null, "b": [1, 2]})), vec!["$.a".to_string()]);
}
```

  В `environment.json` — пять переменных: обычная с `type: "default"`, `type: "secret"`, `enabled: false`,
  `num` с числовым значением `5` без ключа `type`, `nil` со значением `null`; плюс
  `_postman_variable_scope: "environment"` и `_postman_exported_at`. В `collection_small.json` у `R1` в
  `protocolProfileBehavior` ключа `disabledSystemHeaders` нет, у `graphql`-тела `R6` нет `operationName`. В `globals.json` — две переменные и
  `_postman_variable_scope: "globals"`.

- [ ] **Step 2: `cargo test -p rqm-postman`** — ошибка компиляции.
- [ ] **Step 3: Реализовать `parse_text`, `detect`, `import_collection`, `import_environment`, `ImportReport`,
  `semantic_diff`.** Разбор — по `serde_json::Value` с терпимым чтением (неверный тип значения = «поля нет»);
  обход для `semantic_diff` взять из `spike/postman-probe/src/lib.rs`, но в результат класть только путь (`$.a`,
  `$.b[0]`) без пояснений в скобках; при разной длине массивов — путь самого массива.
- [ ] **Step 4: `cargo test -p rqm-postman` и `cargo clippy -p rqm-postman -- -D warnings`** — 12 passed.
- [ ] **Step 5: Commit** — «Этап 1A: импорт коллекций и окружений Postman (rqm-postman)».

---

### Task 3: `rqm-postman` — экспорт, слияние globals, образец владельца

**Files:**
- Create: `crates/rqm-postman/src/export.rs`, `src/globals_merge.rs`
- Modify: `crates/rqm-postman/src/lib.rs` (экспорт новых функций)
- Test: `crates/rqm-postman/tests/export.rs`, `crates/rqm-postman/tests/owner_samples.rs`

**Interfaces:**
- Consumes: Task 1, Task 2.
- Produces:

```rust
pub enum ExportError { NoCollectionRoot, OrphanNode(NodeId) }
/// `nodes` = one collection subtree in any order; children are ordered by `sort_order`.
pub fn export_collection(nodes: &[Node]) -> Result<serde_json::Value, ExportError>;
pub fn export_environment(env: &Environment) -> serde_json::Value;

/// Names present in both lists with different values (enabled or not), in `incoming` order.
pub fn globals_conflicts(current: &[Variable], incoming: &[Variable]) -> Vec<String>;
pub enum MergePolicy { TakeIncoming, KeepCurrent }
/// Current order is kept, new variables are appended in `incoming` order.
pub fn merge_globals(current: &[Variable], incoming: &[Variable], policy: MergePolicy) -> Vec<Variable>;
```

**Правила экспорта:** P1–P4 из Global Constraints, плюс:
- У узла без `postman_extra` (создан в приложении) всё генерируется: `info` = `{name, schema}` со схемой
  `https://schema.getpostman.com/json/collection/v2.1.0/collection.json` и новым `_postman_id`; у запроса —
  `request` объектом.
- `url` всегда генерируется объектом `{raw, protocol, host, port, path, query, variable, hash}` — и у нового
  узла, и при перезаписи изменённого `url` по P2. Библиотеки Postman поле `raw` не читают, адрес собирают из
  частей, поэтому части обязательны. Разбор адреса (как `Url.parse` в postman-collection): `hash` — часть после
  `#`; `protocol` — часть до `://` (ключ не пишется, если схемы нет); дальше до первого `/` — хост: `port` —
  цифры после последнего `:` (ключ не пишется, если порта нет), остальное режется по `.` в массив `host`;
  остаток режется по `/` в массив `path`. `{{base}}/x` → `host: ["{{base}}"]`, `path: ["x"]`. Пустые `query`,
  `variable`, `hash`, `path` не пишутся. `raw` = адрес без `#…` + `?` + включённые параметры `key=value` через
  `&` + `#hash`.
- `name` всегда берётся из `Node.name`; для коллекции пишется в `info.name`.
- Когда типизированное поле изменено, элементы списков генерируются по P3; у элемента без `postman_extra` —
  ключи `key`, `value` и `disabled: true` только у отключённых (у окружений — `enabled` и `type`).
- `disabled_auto_headers` пишется в `protocolProfileBehavior.disabledSystemHeaders` как `{имя: true}`;
  пустой список — ключ не пишется.
- `Auth::Unsupported` никогда не генерируется заново — остаётся исходный `auth` из `postman_extra`.
- `auth = None`: если в исходнике был `"auth": null` — остаётся `null`; если ключа не было — ключ не пишется;
  если в исходнике был тип, а в модели стало `None` — ключ удаляется.
- `FileRef.confirmed` в файл Postman не пишется.
- Окружение: `values` по P2/P3; `_postman_variable_scope` — по `EnvKind`.
- `globals_conflicts` и `merge_globals`: конфликт — совпало имя, значения разные **[моё решение]**; если имя и
  значение совпали, остаётся текущая переменная целиком (признаки и `postman_extra` не трогаются); при
  `TakeIncoming` переменная из файла заменяет текущую целиком; при повторе имени внутри одного списка
  сравнивается последняя.

- [ ] **Step 1: Написать падающие тесты `tests/export.rs`**

```rust
use rqm_model::*;
use rqm_postman::*;

fn load(name: &str) -> serde_json::Value {
    parse_text(&std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap()).unwrap()
}
fn req_mut<'a>(nodes: &'a mut [Node], name: &str) -> &'a mut RequestContent {
    match &mut nodes.iter_mut().find(|n| n.name == name).unwrap().content { NodeContent::Request(r) => r, _ => panic!() }
}
fn item<'a>(v: &'a serde_json::Value, path: &[&str]) -> &'a serde_json::Value {
    path.iter().fold(v, |cur, name| cur["item"].as_array().unwrap().iter().find(|i| i["name"] == *name).unwrap())
}

#[test] fn untouched_collection_round_trips_exactly() {
    let src = load("collection_small.json");
    let out = export_collection(&import_collection(&src).unwrap().nodes).unwrap();
    assert_eq!(semantic_diff(&src, &out), Vec::<String>::new());
}
#[test] fn untouched_environment_and_globals_round_trip() {
    for f in ["environment.json", "globals.json"] {
        let src = load(f);
        assert_eq!(semantic_diff(&src, &export_environment(&import_environment(&src).unwrap())), Vec::<String>::new());
    }
}
#[test] fn editing_one_header_keeps_everything_else() {
    let src = load("collection_small.json");
    let mut nodes = import_collection(&src).unwrap().nodes;
    let idx = req_mut(&mut nodes, "R1").headers.iter().position(|h| !h.postman_extra.contains_key("description")).unwrap();
    req_mut(&mut nodes, "R1").headers[idx].value = "changed".into();
    let out = export_collection(&nodes).unwrap();
    let diff = semantic_diff(&src, &out);
    assert_eq!(diff.len(), 1, "only the edited header value may differ");
    assert!(diff[0].ends_with(&format!(".request.header[{idx}].value")));
    let r1 = item(&out, &["F1", "F2", "R1"]);
    assert!(r1["request"]["header"].as_array().unwrap().iter().any(|h| h.get("description").is_some()));
    assert_eq!(r1["request"]["url"], item(&src, &["F1", "F2", "R1"])["request"]["url"]);
}
#[test] fn explicit_null_auth_survives() {
    let src = load("collection_small.json");
    let out = export_collection(&import_collection(&src).unwrap().nodes).unwrap();
    assert!(item(&out, &["R3"])["request"].get("auth").is_some_and(|a| a.is_null()));
}
#[test] fn unsupported_auth_is_never_rewritten() {
    let src = load("collection_small.json");
    let mut nodes = import_collection(&src).unwrap().nodes;
    req_mut(&mut nodes, "R4").method = "PUT".into();
    let out = export_collection(&nodes).unwrap();
    assert_eq!(item(&out, &["R4"])["request"]["auth"], item(&src, &["R4"])["request"]["auth"]);
    assert_eq!(item(&out, &["R4"])["request"]["method"], "PUT");
}
#[test] fn editing_a_script_replaces_only_exec() {
    let src = load("collection_small.json");
    let mut nodes = import_collection(&src).unwrap().nodes;
    req_mut(&mut nodes, "R1").scripts.test = "a\nb".into();
    let out = export_collection(&nodes).unwrap();
    let ev = &item(&out, &["F1", "F2", "R1"])["event"][0];
    assert_eq!(ev["script"]["exec"], serde_json::json!(["a", "b"]));
    assert_eq!(ev["script"]["type"], item(&src, &["F1", "F2", "R1"])["event"][0]["script"]["type"]);
}
#[test] fn rename_and_reorder_are_exported() {
    let mut nodes = import_collection(&load("collection_small.json")).unwrap().nodes;
    nodes.iter_mut().find(|n| n.name == "R3").unwrap().sort_order = 99;
    nodes.iter_mut().find(|n| n.name == "R5").unwrap().name = "R5 renamed".into();
    let out = export_collection(&nodes).unwrap();
    let names: Vec<_> = out["item"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, vec!["F1", "R4", "R5 renamed", "R6", "R3"]);
}
#[test] fn node_created_in_app_exports_valid_v21() {
    let root = Node { id: new_id(), parent_id: None, kind: NodeKind::Collection, name: "New".into(), sort_order: 0,
        content: NodeContent::Collection(CollectionContent { auth: None, variables: vec![], scripts: Scripts::default(), postman_extra: Extra::new() }) };
    let r = Node { id: new_id(), parent_id: Some(root.id.clone()), kind: NodeKind::Request, name: "Get".into(), sort_order: 0,
        content: NodeContent::Request(RequestContent { method: "GET".into(), url: "https://example.test/x".into(),
            query: vec![Param { key: "a".into(), value: "1".into(), enabled: true, postman_extra: Extra::new() },
                        Param { key: "b".into(), value: "2".into(), enabled: false, postman_extra: Extra::new() }],
            path_variables: vec![], headers: vec![], body: Body::None, auth: Some(Auth::Bearer { token: "t".into() }),
            scripts: Scripts::default(), settings: RequestSettings::default(), disabled_auto_headers: vec!["accept-encoding".into()],
            postman_extra: Extra::new() }) };
    let out = export_collection(&[r, root]).unwrap();
    assert_eq!(out["item"][0]["protocolProfileBehavior"]["disabledSystemHeaders"], serde_json::json!({"accept-encoding": true}));
    assert!(matches!(detect(&out), Ok(Detected::Collection)));
    assert_eq!(out["info"]["name"], "New");
    let url = &out["item"][0]["request"]["url"];
    assert_eq!(url["raw"], "https://example.test/x?a=1");
    assert_eq!((&url["protocol"], &url["host"], &url["path"]), (&serde_json::json!("https"), &serde_json::json!(["example", "test"]), &serde_json::json!(["x"])));
    assert!(url.get("port").is_none() && url.get("hash").is_none());
    assert_eq!(out["item"][0]["request"]["url"]["query"].as_array().unwrap().len(), 2);
    assert_eq!(out["item"][0]["request"]["auth"]["type"], "bearer");
    // and it imports back to the same typed content
    let back = import_collection(&out).unwrap();
    assert_eq!(back.nodes.len(), 2);
    let NodeContent::Request(b) = &back.nodes[1].content else { panic!() };
    assert_eq!((b.url.as_str(), b.query.len(), b.disabled_auto_headers.len()), ("https://example.test/x", 2, 1));
}
#[test] fn generated_url_parts() {
    let mut nodes = import_collection(&load("collection_small.json")).unwrap().nodes;
    req_mut(&mut nodes, "R5").url = "{{base}}:8080/a/b#frag".into();
    let out = export_collection(&nodes).unwrap();
    let url = &item(&out, &["R5"])["request"]["url"];
    assert!(url.get("protocol").is_none());
    assert_eq!((&url["host"], &url["port"], &url["path"], &url["hash"]),
        (&serde_json::json!(["{{base}}"]), &serde_json::json!("8080"), &serde_json::json!(["a", "b"]), &serde_json::json!("frag")));
}
#[test] fn editing_one_environment_variable_keeps_the_others() {
    let src = load("environment.json");
    let mut e = import_environment(&src).unwrap();
    let idx = e.variables.iter().position(|v| v.secret).unwrap();
    e.variables[idx].value = "changed".into();
    let diff = semantic_diff(&src, &export_environment(&e));
    assert_eq!(diff, vec![format!("$.values[{idx}].value")]);     // the number, the null and the missing `type` survive
}
#[test] fn export_errors() {
    assert!(matches!(export_collection(&[]), Err(ExportError::NoCollectionRoot)));
}
#[test] fn globals_merge_rules() {
    let v = |k: &str, val: &str| Variable { key: k.into(), value: val.into(), secret: false, enabled: true, postman_extra: Extra::new() };
    let cur = vec![v("a", "1"), v("b", "2")];
    let inc = vec![v("b", "20"), v("a", "1"), v("c", "3")];
    assert_eq!(globals_conflicts(&cur, &inc), vec!["b".to_string()]);
    let take: Vec<_> = merge_globals(&cur, &inc, MergePolicy::TakeIncoming).into_iter().map(|x| (x.key, x.value)).collect();
    assert_eq!(take, vec![("a".into(), "1".into()), ("b".into(), "20".into()), ("c".into(), "3".into())]);
    let keep: Vec<_> = merge_globals(&cur, &inc, MergePolicy::KeepCurrent).into_iter().map(|x| (x.key, x.value)).collect();
    assert_eq!(keep, vec![("a".into(), "1".into()), ("b".into(), "2".into()), ("c".into(), "3".into())]);
}
```

- [ ] **Step 2: Написать тест на образцах владельца `tests/owner_samples.rs`**

```rust
/// Every `*.json` in `<repo>/spike/samples/` must import and export back with an empty semantic diff.
/// Prints counts and JSON paths only — never values from the owner's files.
#[test] fn owner_samples_round_trip() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spike/samples");
    let files: Vec<_> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json")).collect()).unwrap_or_default();
    if files.is_empty() { println!("owner samples: 0 (skipped)"); return; }
    let mut checked = 0;
    for (n, path) in files.iter().enumerate() {
        let Ok(src) = rqm_postman::parse_text(&std::fs::read_to_string(path).unwrap()) else { continue };
        let Ok(kind) = rqm_postman::detect(&src) else { continue };          // unsupported format: counted as skipped
        let (out, summary) = match kind {
            rqm_postman::Detected::Collection => {
                let c = rqm_postman::import_collection(&src).expect("imports");
                (rqm_postman::export_collection(&c.nodes).expect("exports"),
                 format!("nodes {}, report items {}", c.nodes.len(), c.report.items.len()))
            }
            _ => { let e = rqm_postman::import_environment(&src).expect("imports");
                   (rqm_postman::export_environment(&e), format!("variables {}", e.variables.len())) }
        };
        let diff = rqm_postman::semantic_diff(&src, &out);
        println!("owner sample #{n}: {summary}, diff paths {}", diff.len());
        assert!(diff.is_empty(), "sample #{n} differs at {} paths", diff.len());
        checked += 1;
    }
    println!("owner samples: {checked}, skipped: {}", files.len() - checked);
}
```

- [ ] **Step 3: `cargo test -p rqm-postman`** — ошибка компиляции.
- [ ] **Step 4: Реализовать `export_collection`, `export_environment`, `globals_conflicts`, `merge_globals`.**
- [ ] **Step 5: `cargo test -p rqm-postman -- --nocapture` и clippy** — 12 + 12 + 1 passed; для текущего
  образца в выводе `owner samples: 1, skipped: 0` и `nodes 47` (1 коллекция + 3 папки + 43 запроса); числа
  изменятся, когда владелец добавит файлы.
- [ ] **Step 6: Commit** — «Этап 1A: экспорт в формат Postman и слияние globals».

---

### Task 4: `rqm-vars` — области и подстановка

**Files:**
- Create: `crates/rqm-vars/Cargo.toml`, `src/lib.rs`, `src/dynamic.rs`
- Test: `crates/rqm-vars/tests/substitute.rs`

**Interfaces:**
- Consumes: ничего из других crates (зависимости: `uuid`, `regex` не нужен — разбор вручную).
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind { Global, Collection, Environment, Local }
pub struct Scope { pub kind: ScopeKind, pub values: Vec<(String, String)> }   // enabled variables only

pub struct Resolver { /* private */ }
impl Resolver {
    /// Scopes from the widest to the narrowest; a narrower scope wins. Within one scope the last entry wins.
    pub fn new(scopes: Vec<Scope>) -> Self;
    pub fn lookup(&self, name: &str) -> Option<(&str, ScopeKind)>;
    /// Uses the system clock and random generator for `{{$...}}`.
    pub fn substitute(&self, input: &str) -> Substituted;
    /// `dynamic(name)` is called once per occurrence of `{{$name}}`; `None` leaves the token as is.
    pub fn substitute_with(&self, input: &str, dynamic: &mut dyn FnMut(&str) -> Option<String>) -> Substituted;
}
pub struct Substituted { pub text: String, pub unresolved: Vec<String> }   // names without braces, deduplicated, in order

/// Replaces `/:name` path segments with enabled path-variable values. The name runs from `:` to the first `.`
/// or to the end of the segment (next `/`, `?` or `#`); the tail after the dot is kept (`/:id.json` -> `/7.json`).
/// A segment without a matching variable, or whose variable has an empty value, stays as is.
pub fn apply_path_variables(url: &str, vars: &[(String, String)]) -> String;
/// `guid`, `randomUUID` (UUID v4), `timestamp` (Unix seconds), `isoTimestamp` (UTC, `YYYY-MM-DDTHH:MM:SS.mmmZ`),
/// `randomInt` (0..=1000). Unknown name → `None`.
pub fn system_dynamic(name: &str) -> Option<String>;
```

**Алгоритм подстановки** (спека, раздел 5): токен — `{{имя}}`, где имя не содержит `{` и `}`. За один проход
заменяются все самые внутренние токены слева направо; проходы повторяются, пока текст меняется, но не больше
19 раз. Имя ищется сначала среди переменных, затем, если начинается с `$`, — среди динамических (без `$`).
Ненайденный токен остаётся в тексте как есть и попадает в `unresolved`. Имя берётся дословно, без обрезки
пробелов. Всё это сверено с исходниками postman-collection (`lib/superstring`).

- [ ] **Step 1: Написать падающие тесты `tests/substitute.rs`**

```rust
use rqm_vars::*;

fn r(pairs: &[(ScopeKind, &str, &str)]) -> Resolver {
    let mut scopes: Vec<Scope> = [ScopeKind::Global, ScopeKind::Collection, ScopeKind::Environment, ScopeKind::Local]
        .into_iter().map(|kind| Scope { kind, values: vec![] }).collect();
    for (k, n, v) in pairs { scopes.iter_mut().find(|s| s.kind == *k).unwrap().values.push((n.to_string(), v.to_string())); }
    Resolver::new(scopes)
}
fn no_dyn(_: &str) -> Option<String> { None }
fn sub(res: &Resolver, s: &str) -> (String, Vec<String>) { let o = res.substitute_with(s, &mut no_dyn); (o.text, o.unresolved) }

#[test] fn table_of_cases() {
    use ScopeKind::*;
    let res = r(&[(Global, "host", "g.test"), (Collection, "host", "c.test"), (Environment, "host", "e.test"),
                  (Global, "only_g", "G"), (Environment, "b", "host"), (Environment, "tpl", "{{host}}/x"),
                  (Environment, "empty", ""), (Environment, "sp ace", "S"), (Local, "loc", "L")]);
    let cases: &[(&str, &str, &[&str])] = &[
        ("{{host}}", "e.test", &[]),
        ("{{only_g}}-{{loc}}", "G-L", &[]),
        ("a{{host}}b{{host}}c", "ae.testbe.testc", &[]),
        ("{{{{b}}}}", "e.test", &[]),                 // inner token first
        ("{{tpl}}", "e.test/x", &[]),                 // value containing a token
        ("{{missing}}", "{{missing}}", &["missing"]),
        ("{{missing}} {{missing}}", "{{missing}} {{missing}}", &["missing"]),
        ("{{empty}}|", "|", &[]),
        ("{{sp ace}}", "S", &[]),
        ("{{ host }}", "{{ host }}", &[" host "]),
        ("{host} {{host}", "{host} {{host}", &[]),
        ("{{}}", "{{}}", &[""]),
        ("", "", &[]),
        ("плюс {{host}} ✓", "плюс e.test ✓", &[]),
    ];
    for (input, text, unresolved) in cases {
        let (t, u) = sub(&res, input);
        assert_eq!(&t, text, "input {input:?}");
        assert_eq!(u, unresolved.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "input {input:?}");
    }
}
#[test] fn narrower_scope_wins_and_lookup_reports_scope() {
    use ScopeKind::*;
    let res = r(&[(Global, "x", "1"), (Collection, "x", "2")]);
    assert_eq!(res.lookup("x"), Some(("2", Collection)));
    assert_eq!(res.lookup("nope"), None);
    let res = r(&[(Global, "x", "1"), (Local, "x", "4"), (Environment, "x", "3")]);
    assert_eq!(res.lookup("x"), Some(("4", Local)));
}
#[test] fn cycles_terminate() {
    use ScopeKind::*;
    let res = r(&[(Global, "a", "{{a}}"), (Global, "p", "{{q}}"), (Global, "q", "{{p}}"), (Global, "grow", "{{grow}}{{grow}}")]);
    assert_eq!(sub(&res, "{{a}}").0, "{{a}}");
    let (t, _) = sub(&res, "{{p}}");
    assert!(t == "{{p}}" || t == "{{q}}");
    let started = std::time::Instant::now();
    let (t, _) = sub(&res, "{{grow}}");
    assert!(t.len() <= 8 * (1 << 19));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}
#[test] fn pass_limit_is_19() {
    use ScopeKind::*;
    // v0 -> {{v1}} -> ... -> {{v25}} -> "end": 26 levels, more than 19 passes
    let pairs: Vec<(String, String)> = (0..26).map(|i| (format!("v{i}"), if i == 25 { "end".into() } else { format!("{{{{v{}}}}}", i + 1) })).collect();
    let res = Resolver::new(vec![Scope { kind: Global, values: pairs }]);
    assert_eq!(sub(&res, "{{v7}}").0, "end");          // 19 passes are enough from v7
    assert_eq!(sub(&res, "{{v0}}").0, "{{v19}}");      // stops after 19 passes
}
#[test] fn dynamic_is_called_per_occurrence_and_user_variable_wins() {
    use ScopeKind::*;
    let mut n = 0;
    let mut dynamic = |name: &str| { n += 1; (name == "guid").then(|| format!("G{n}")) };
    let res = r(&[(Global, "$mine", "user")]);
    let out = res.substitute_with("{{$guid}}/{{$guid}}/{{$mine}}/{{$unknown}}", &mut dynamic);
    assert_eq!(out.text, "G1/G2/user/{{$unknown}}");
    assert_eq!(out.unresolved, vec!["$unknown".to_string()]);
}
#[test] fn system_dynamic_values_have_the_right_shape() {
    for name in ["guid", "randomUUID"] {
        let a = system_dynamic(name).unwrap();
        assert_eq!(a.len(), 36);
        assert_ne!(a, system_dynamic(name).unwrap());
    }
    assert!(system_dynamic("timestamp").unwrap().parse::<u64>().unwrap() > 1_700_000_000);
    let iso = system_dynamic("isoTimestamp").unwrap();
    assert_eq!(iso.len(), 24);
    assert!(iso.ends_with('Z') && iso.as_bytes()[10] == b'T');
    for _ in 0..200 { assert!(system_dynamic("randomInt").unwrap().parse::<u32>().unwrap() <= 1000); }
    assert_eq!(system_dynamic("nope"), None);
}
#[test] fn path_variables() {
    let v = vec![("id".to_string(), "7".to_string()), ("name".to_string(), "a b".to_string())];
    assert_eq!(apply_path_variables("https://h.test/u/:id/x/:name?q=:id#:id", &v), "https://h.test/u/7/x/a b?q=:id#:id");
    assert_eq!(apply_path_variables("https://h.test:8080/:missing/:id", &v), "https://h.test:8080/:missing/7");
    assert_eq!(apply_path_variables("{{base}}/:id", &v), "{{base}}/7");
    assert_eq!(apply_path_variables("https://h.test/:idx", &v), "https://h.test/:idx");
    assert_eq!(apply_path_variables("https://h.test/:id.json", &v), "https://h.test/7.json");
    assert_eq!(apply_path_variables("https://h.test/:e/x", &[("e".to_string(), String::new())]), "https://h.test/:e/x");
}
```

- [ ] **Step 2: `cargo test -p rqm-vars`** — ошибка компиляции.
- [ ] **Step 3: Реализовать `Resolver`, `apply_path_variables`, `system_dynamic`.** `isoTimestamp` — без crate
  `chrono`: посчитать дату из `SystemTime` вручную (алгоритм civil-from-days) и покрыть его модульным тестом
  на три известные даты, включая 29 февраля.
- [ ] **Step 4: `cargo test -p rqm-vars` и clippy** — 7 passed (плюс модульный тест даты).
- [ ] **Step 5: Commit** — «Этап 1A: области переменных и подстановка (rqm-vars)».

---

### Task 5: `rqm-store` — SQLite

**Files:**
- Create: `crates/rqm-store/Cargo.toml`, `src/lib.rs`, `src/schema.sql`, `src/nodes.rs`, `src/environments.rs`,
  `src/state.rs`
- Test: `crates/rqm-store/tests/store.rs`

**DDL (`src/schema.sql`) — согласуется владельцем вместе с этим планом, менять нельзя:**

```sql
CREATE TABLE node (
    -- Дерево коллекций: каждая строка — коллекция, папка или запрос.
    -- Читают: дерево в окне, отправка запроса (rqm-runner), экспорт в Postman.

    -- Идентификатор узла: UUID, создаёт приложение при импорте или создании узла. На него ссылаются потомки.
    id         TEXT PRIMARY KEY,
    -- Родитель: папка или коллекция. NULL только у коллекции. Удаление родителя удаляет потомков.
    parent_id  TEXT REFERENCES node(id) ON DELETE CASCADE,
    -- Вид узла; задаётся при создании и не меняется. От него зависит, как читать content.
    kind       TEXT NOT NULL CHECK (kind IN ('collection', 'folder', 'request')),
    -- Имя, которое видно в дереве; из Postman (info.name / item.name) или введено пользователем.
    name       TEXT NOT NULL,
    -- Порядок среди узлов одного родителя (0, 1, 2…); из порядка в файле Postman, меняется перетаскиванием.
    -- У коллекций — порядок в списке коллекций: новая и импортированная становятся последними.
    sort_order INTEGER NOT NULL,
    -- Содержимое узла в JSON (rqm-model): для запроса — метод, URL, параметры, заголовки, тело, auth, скрипты,
    -- настройки; для папки и коллекции — auth, скрипты, у коллекции ещё переменные. Внутри же лежит postman_extra —
    -- исходный фрагмент Postman, нужный для экспорта без потерь. Пишется при импорте и при сохранении редактора.
    content    TEXT NOT NULL,
    CHECK ((kind = 'collection') = (parent_id IS NULL))
) STRICT;

CREATE TABLE environment (
    -- Окружения и глобальные переменные.
    -- Читают: переключатель окружений, подстановка переменных при отправке, экспорт в Postman.

    -- Идентификатор: UUID от приложения. У единственной строки глобальных переменных — постоянное значение
    -- 'globals'; по нему же строка отличается от обычных окружений, отдельной колонки вида нет.
    id      TEXT PRIMARY KEY,
    -- Имя в переключателе окружений; из файла Postman или введено пользователем.
    name    TEXT NOT NULL,
    -- Переменные в JSON (rqm-model): имя, значение, признак секрета, включена ли; плюс postman_extra для экспорта.
    -- Пишется при импорте, из редактора окружений и (с этапа 2) из скриптов.
    content TEXT NOT NULL
) STRICT;

CREATE TABLE app_state (
    -- Состояние приложения «ключ — значение».
    -- На этапе 1A используется один ключ: 'cookie_jar' — cookies, полученные в ответах; пишет и читает rqm-runner.
    -- Ключи состояния окна (активное окружение, вкладки, черновики) добавит план 1B, новых колонок для них не нужно.

    -- Имя ключа; задаётся кодом приложения (константа в том crate, который владеет ключом).
    key   TEXT PRIMARY KEY,
    -- Значение в JSON; пишет и разбирает код-владелец ключа.
    value TEXT NOT NULL
) STRICT;
```

Комментарии таблиц стоят внутри скобок, чтобы SQLite сохранил их вместе со схемой (текст до `CREATE` в базу не
попадает). Колонки вида у окружения нет **[моё решение]** (в таблице раздела 5 спеки «вид» назван): вид полностью
определяется значением `id`, отдельного читателя у колонки нет.

Индексов нет: на 3000 узлов полный просмотр таблицы быстрее миллисекунды, отдельного читателя у индекса нет.
Версия схемы — `PRAGMA user_version = 1` (таблица миграций не создаётся). При открытии: `user_version = 0` и
нет таблиц → создать схему; `1` → работать; больше `1` → ошибка `StoreError::NewerSchema`. На каждом
соединении — `PRAGMA foreign_keys = ON`. Метод запроса для цветной метки в дереве берётся из `content`
функцией `json_extract(content, '$.method')` — отдельной колонки нет.

**Interfaces:**
- Consumes: Task 1.
- Produces:

```rust
pub enum StoreError { Sqlite(String), Io(String), NewerSchema(i64), NotFound(String), InvalidMove(String),
                      CorruptContent { id: String, error: String } }
pub struct TreeRow { pub id: NodeId, pub parent_id: Option<NodeId>, pub kind: NodeKind, pub name: String,
                     pub sort_order: i64, pub method: Option<String> }

pub struct Store { /* rusqlite::Connection */ }
impl Store {
    /// Creates parent directories and the file when missing.
    pub fn open(path: &std::path::Path) -> Result<Self, StoreError>;
    pub fn open_in_memory() -> Result<Self, StoreError>;
    /// `%LOCALAPPDATA%\RequestMaster\app.db`.
    pub fn default_path() -> Result<std::path::PathBuf, StoreError>;

    /// One transaction; used by import. Parents must precede children. A root node (collection) ignores its own
    /// `sort_order` and becomes the last collection.
    pub fn insert_nodes(&mut self, nodes: &[Node]) -> Result<(), StoreError>;
    /// All nodes without content, ordered by (parent_id, sort_order).
    pub fn tree(&self) -> Result<Vec<TreeRow>, StoreError>;
    pub fn node(&self, id: &str) -> Result<Node, StoreError>;                       // NotFound
    /// Root first, every parent before its children, siblings by sort_order.
    pub fn subtree(&self, root_id: &str) -> Result<Vec<Node>, StoreError>;
    /// From the collection down to the direct parent; empty for a collection.
    pub fn ancestors(&self, id: &str) -> Result<Vec<Node>, StoreError>;
    /// Appends as the last child of `parent_id` (or as the last collection); returns the new id.
    pub fn create_node(&mut self, parent_id: Option<&str>, kind: NodeKind, name: &str, content: &NodeContent) -> Result<NodeId, StoreError>;
    pub fn rename_node(&mut self, id: &str, name: &str) -> Result<(), StoreError>;
    pub fn update_content(&mut self, id: &str, content: &NodeContent) -> Result<(), StoreError>;
    /// Moves under `new_parent` at position `index` (clamped) and renumbers siblings 0..n in both parents.
    pub fn move_node(&mut self, id: &str, new_parent: Option<&str>, index: usize) -> Result<(), StoreError>;
    /// Deletes the node with its descendants and renumbers the remaining siblings 0..n (one transaction).
    pub fn delete_node(&mut self, id: &str) -> Result<(), StoreError>;
    /// Deep copy with fresh ids right after the original; the copy's name gets the suffix " Copy".
    pub fn duplicate_node(&mut self, id: &str) -> Result<NodeId, StoreError>;

    /// Environments ordered by `name.to_lowercase()` — sorted in Rust, because SQLite `NOCASE` folds ASCII only;
    /// globals are not included.
    pub fn environments(&self) -> Result<Vec<Environment>, StoreError>;
    /// Stored globals, or `Environment::empty_globals()` when nothing is stored yet.
    pub fn globals(&self) -> Result<Environment, StoreError>;
    pub fn save_environment(&mut self, env: &Environment) -> Result<(), StoreError>;   // insert or replace by id
    pub fn delete_environment(&mut self, id: &str) -> Result<(), StoreError>;

    pub fn state_get(&self, key: &str) -> Result<Option<String>, StoreError>;
    pub fn state_set(&mut self, key: &str, value: &str) -> Result<(), StoreError>;
}
```

  Правила `move_node` / `create_node` (`InvalidMove`): у запроса не бывает потомков; коллекция — только корень
  (`new_parent = None`) и только она; папку и запрос нельзя сделать корнем; нельзя переместить узел в себя или
  в своего потомка. Любая операция, меняющая несколько строк, — в одной транзакции.

- [ ] **Step 1: Написать падающие тесты `tests/store.rs`**

```rust
use rqm_model::*;
use rqm_store::*;

fn folder() -> NodeContent { NodeContent::Folder(FolderContent { auth: None, scripts: Scripts::default(), postman_extra: Extra::new() }) }
fn collection() -> NodeContent { NodeContent::Collection(CollectionContent { auth: None, variables: vec![], scripts: Scripts::default(), postman_extra: Extra::new() }) }
fn request(method: &str) -> NodeContent { NodeContent::Request(RequestContent { method: method.into(), url: "https://example.test".into(),
    query: vec![], path_variables: vec![], headers: vec![], body: Body::None, auth: None, scripts: Scripts::default(),
    settings: RequestSettings::default(), disabled_auto_headers: vec![], postman_extra: Extra::new() }) }
/// C ─ F1 ─ F2 ─ R1 ; C ─ R2 ; C ─ R3
fn seeded() -> (Store, [NodeId; 6]) {
    let mut s = Store::open_in_memory().unwrap();
    let c = s.create_node(None, NodeKind::Collection, "C", &collection()).unwrap();
    let f1 = s.create_node(Some(&c), NodeKind::Folder, "F1", &folder()).unwrap();
    let f2 = s.create_node(Some(&f1), NodeKind::Folder, "F2", &folder()).unwrap();
    let r1 = s.create_node(Some(&f2), NodeKind::Request, "R1", &request("POST")).unwrap();
    let r2 = s.create_node(Some(&c), NodeKind::Request, "R2", &request("GET")).unwrap();
    let r3 = s.create_node(Some(&c), NodeKind::Request, "R3", &request("DELETE")).unwrap();
    (s, [c, f1, f2, r1, r2, r3])
}
fn children(s: &Store, parent: &str) -> Vec<(String, i64)> {
    s.tree().unwrap().into_iter().filter(|r| r.parent_id.as_deref() == Some(parent)).map(|r| (r.name, r.sort_order)).collect()
}

#[test] fn schema_has_exactly_three_tables_and_version_1() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("app.db");
    drop(Store::open(&path).unwrap());
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mut names: Vec<String> = conn.prepare("SELECT name FROM sqlite_master WHERE type IN ('table','index') AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap()
        .query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    names.sort();
    assert_eq!(names, vec!["app_state", "environment", "node"]);
    assert_eq!(conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    for table in ["node", "environment", "app_state"] {
        let sql: String = conn.query_row("SELECT sql FROM sqlite_master WHERE name = ?1", [table], |r| r.get(0)).unwrap();
        assert!(sql.matches("--").count() >= 4, "{table}: table and column comments must be stored with the schema");
    }
    let cols: Vec<String> = conn.prepare("SELECT name FROM pragma_table_info('environment')").unwrap().query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    assert_eq!(cols, vec!["id", "name", "content"]);
}
#[test] fn reopen_keeps_data_and_newer_schema_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    { let mut s = Store::open(&path).unwrap(); s.create_node(None, NodeKind::Collection, "Привет", &collection()).unwrap(); }
    assert_eq!(Store::open(&path).unwrap().tree().unwrap()[0].name, "Привет");
    rusqlite::Connection::open(&path).unwrap().execute_batch("PRAGMA user_version = 2").unwrap();
    assert!(matches!(Store::open(&path), Err(StoreError::NewerSchema(2))));
}
#[test] fn database_path_with_non_ascii_characters_works() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Профиль Ӯ 测试").join("app.db");
    { let mut s = Store::open(&path).unwrap(); s.state_set("k", "значение").unwrap(); }
    assert_eq!(Store::open(&path).unwrap().state_get("k").unwrap().as_deref(), Some("значение"));
}
#[test] fn default_path_is_under_local_app_data() {
    let p = Store::default_path().unwrap();
    assert!(p.ends_with("RequestMaster/app.db") || p.ends_with("RequestMaster\\app.db"));
    assert!(p.starts_with(std::env::var("LOCALAPPDATA").unwrap()));
}
#[test] fn tree_rows_carry_method_and_order() {
    let (s, [c, ..]) = seeded();
    assert_eq!(children(&s, &c), vec![("F1".into(), 0), ("R2".into(), 1), ("R3".into(), 2)]);
    let rows = s.tree().unwrap();
    assert_eq!(rows.iter().find(|r| r.name == "R1").unwrap().method.as_deref(), Some("POST"));
    assert_eq!(rows.iter().find(|r| r.name == "F1").unwrap().method, None);
}
#[test] fn subtree_and_ancestors() {
    let (s, [c, f1, f2, r1, ..]) = seeded();
    let names: Vec<_> = s.subtree(&f1).unwrap().into_iter().map(|n| n.name).collect();
    assert_eq!(names, vec!["F1", "F2", "R1"]);
    assert_eq!(s.subtree(&c).unwrap().len(), 6);
    let anc: Vec<_> = s.ancestors(&r1).unwrap().into_iter().map(|n| n.id).collect();
    assert_eq!(anc, vec![c.clone(), f1, f2]);
    assert!(s.ancestors(&c).unwrap().is_empty());
    assert!(matches!(s.node("missing"), Err(StoreError::NotFound(_))));
}
#[test] fn insert_nodes_is_atomic() {
    let mut s = Store::open_in_memory().unwrap();
    let root = Node { id: new_id(), parent_id: None, kind: NodeKind::Collection, name: "C".into(), sort_order: 0, content: collection() };
    let orphan = Node { id: new_id(), parent_id: Some("missing".into()), kind: NodeKind::Request, name: "R".into(), sort_order: 0, content: request("GET") };
    assert!(s.insert_nodes(&[root, orphan]).is_err());
    assert!(s.tree().unwrap().is_empty());
}
#[test] fn imported_collections_are_appended_in_order() {
    let mut s = Store::open_in_memory().unwrap();
    for name in ["First", "Second"] {
        let root = Node { id: new_id(), parent_id: None, kind: NodeKind::Collection, name: name.into(), sort_order: 0, content: collection() };
        let child = Node { id: new_id(), parent_id: Some(root.id.clone()), kind: NodeKind::Request, name: "R".into(), sort_order: 0, content: request("GET") };
        s.insert_nodes(&[root, child]).unwrap();
    }
    let roots: Vec<_> = s.tree().unwrap().into_iter().filter(|r| r.parent_id.is_none()).map(|r| (r.name, r.sort_order)).collect();
    assert_eq!(roots, vec![("First".into(), 0), ("Second".into(), 1)]);
}
#[test] fn rename_update_delete() {
    let (mut s, [c, f1, _, r1, ..]) = seeded();
    s.rename_node(&r1, "Новое имя").unwrap();
    s.update_content(&r1, &request("PATCH")).unwrap();
    let n = s.node(&r1).unwrap();
    assert_eq!(n.name, "Новое имя");
    assert!(matches!(n.content, NodeContent::Request(ref r) if r.method == "PATCH"));
    s.delete_node(&f1).unwrap();
    assert_eq!(s.subtree(&c).unwrap().len(), 3);                 // C, R2, R3
    assert_eq!(children(&s, &c), vec![("R2".into(), 0), ("R3".into(), 1)]);
}
#[test] fn move_rules() {
    let (mut s, [c, f1, f2, r1, r2, r3]) = seeded();
    assert!(matches!(s.move_node(&f1, Some(&f2), 0), Err(StoreError::InvalidMove(_))));   // into own descendant
    assert!(matches!(s.move_node(&f1, Some(&f1), 0), Err(StoreError::InvalidMove(_))));   // into itself
    assert!(matches!(s.move_node(&r2, Some(&r3), 0), Err(StoreError::InvalidMove(_))));   // request as a parent
    assert!(matches!(s.move_node(&r2, None, 0), Err(StoreError::InvalidMove(_))));        // request as a root
    assert!(matches!(s.move_node(&c, Some(&f1), 0), Err(StoreError::InvalidMove(_))));    // collection under a folder
    assert_eq!(s.subtree(&c).unwrap().len(), 6, "failed moves change nothing");
    s.move_node(&r3, Some(&c), 0).unwrap();
    assert_eq!(children(&s, &c), vec![("R3".into(), 0), ("F1".into(), 1), ("R2".into(), 2)]);
    s.move_node(&r1, Some(&c), 99).unwrap();
    assert_eq!(children(&s, &c), vec![("R3".into(), 0), ("F1".into(), 1), ("R2".into(), 2), ("R1".into(), 3)]);
    assert!(children(&s, &f2).is_empty());
}
#[test] fn duplicate_is_deep_with_fresh_ids() {
    let (mut s, [c, f1, ..]) = seeded();
    let copy = s.duplicate_node(&f1).unwrap();
    assert_eq!(children(&s, &c), vec![("F1".into(), 0), ("F1 Copy".into(), 1), ("R2".into(), 2), ("R3".into(), 3)]);
    let ids: std::collections::HashSet<_> = s.subtree(&c).unwrap().into_iter().map(|n| n.id).collect();
    assert_eq!(ids.len(), 9);
    assert_eq!(s.subtree(&copy).unwrap().iter().map(|n| n.name.clone()).collect::<Vec<_>>(), vec!["F1 Copy", "F2", "R1"]);
}
#[test] fn environments_and_globals() {
    let mut s = Store::open_in_memory().unwrap();
    assert_eq!(s.globals().unwrap(), Environment::empty_globals());
    let var = Variable { key: "k".into(), value: "v".into(), secret: true, enabled: true, postman_extra: Extra::new() };
    let mut g = Environment::empty_globals(); g.variables.push(var.clone());
    s.save_environment(&g).unwrap();
    s.save_environment(&g).unwrap();
    assert_eq!(s.globals().unwrap().variables, vec![var.clone()]);
    for name in ["beta", "Яблоко", "Alpha", "арбуз"] {
        s.save_environment(&Environment { id: new_id(), kind: EnvKind::Environment, name: name.into(), variables: vec![var.clone()], postman_extra: Extra::new() }).unwrap();
    }
    let envs = s.environments().unwrap();
    assert_eq!(envs.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["Alpha", "beta", "арбуз", "Яблоко"]);
    s.delete_environment(&envs[0].id).unwrap();
    assert_eq!(s.environments().unwrap().len(), 3);
}
#[test] fn app_state_round_trip() {
    let mut s = Store::open_in_memory().unwrap();
    assert_eq!(s.state_get("cookie_jar").unwrap(), None);
    s.state_set("cookie_jar", "[]").unwrap();
    s.state_set("cookie_jar", "[1]").unwrap();
    assert_eq!(s.state_get("cookie_jar").unwrap().as_deref(), Some("[1]"));
}
```

- [ ] **Step 2: `cargo test -p rqm-store`** — ошибка компиляции.
- [ ] **Step 3: Реализовать `Store` по блоку Interfaces; DDL — дословно из этого плана, подключается
  `include_str!("schema.sql")`.** `content` окружения — `Environment::content_json()` / `Environment::from_parts`.
- [ ] **Step 4: `cargo test -p rqm-store` и clippy** — 13 passed.
- [ ] **Step 5: Commit** — «Этап 1A: хранилище SQLite (rqm-store)».

---

### Task 6: `rqm-http` — движок, тело, распаковка, время, отмена

**Files:**
- Create: `crates/rqm-http/Cargo.toml`, `src/lib.rs`, `src/types.rs`, `src/engine.rs`, `src/headers.rs`,
  `src/decode.rs`
- Test: `crates/rqm-http/tests/basic.rs`, `crates/rqm-http/tests/common/mod.rs` (общие помощники тестов)

**Interfaces:**
- Consumes: ничего из других crates.
- Produces:

```rust
pub struct HttpRequest { pub method: String, pub url: String, pub headers: Vec<(String, String)>,
                         pub body: HttpBody, pub options: SendOptions }
pub enum HttpBody { None,
    /// `default_content_type` is sent unless the request has its own `Content-Type` or the auto header is disabled.
    Bytes { data: Vec<u8>, default_content_type: Option<String> },
    Multipart(Vec<MultipartPart>),
    File { path: std::path::PathBuf, default_content_type: Option<String> } }
pub struct MultipartPart { pub name: String, pub value: PartValue, pub content_type: Option<String> }
pub enum PartValue { Text(String), File(std::path::PathBuf) }
pub struct SendOptions { pub timeout: Option<std::time::Duration>, pub follow_redirects: bool,
    pub max_redirects: u32, pub verify_tls: bool, pub proxy: ProxySetting,
    /// Lower-case names of automatic headers the user switched off.
    pub disabled_auto_headers: Vec<String> }
pub enum ProxySetting { System, Off, Manual(String) }   // also PartialEq, Eq, Hash (key of the client cache)
impl Default for SendOptions   // no timeout, follow redirects, max 10, verify TLS, system proxy, nothing disabled

pub struct HttpResponse { pub status: u16, pub status_text: String, pub http_version: String,
    pub headers: Vec<(String, String)>,
    /// Decoded body on disk; the file is owned by the caller after `send` returns.
    pub body_path: std::path::PathBuf,
    pub wire_size: u64, pub decoded_size: u64,
    /// `Some` when the body could not be decoded; the file then holds the body as received.
    pub decode_error: Option<String>,
    pub timings: Timings, pub redirects: Vec<RedirectHop>,
    /// Method, URL and all headers of the last request actually sent, automatic ones included.
    pub sent: SentRequest }
pub struct Timings { pub ttfb: std::time::Duration, pub total: std::time::Duration }
pub struct RedirectHop { pub status: u16, pub method: String, pub url: String, pub location: String }
pub struct SentRequest { pub method: String, pub url: String, pub headers: Vec<(String, String)> }
pub enum HttpError { Cancelled, Timeout, InvalidUrl(String), Connect(String), Tls(String),
    TooManyRedirects(Vec<RedirectHop>), Io(String), Other(String) }

pub trait HttpEngine: Send + Sync {
    fn send<'a>(&'a self, request: HttpRequest, cancel: tokio_util::sync::CancellationToken)
        -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<HttpResponse, HttpError>> + Send + 'a>>;
}
pub struct EngineConfig { pub body_dir: std::path::PathBuf, pub user_agent: String }
pub struct ReqwestEngine { /* private */ }
impl ReqwestEngine { pub fn new(config: EngineConfig) -> Result<Self, HttpError>; }   // Task 7 adds the cookie jar
impl HttpEngine for ReqwestEngine { /* … */ }

/// Automatic headers for display, in this order: Host, User-Agent, Accept, Accept-Encoding, Content-Type,
/// Content-Length. Skips a header the request sets itself (case-insensitive) or lists in
/// `disabled_auto_headers` (`host`, `accept` and `content-length` cannot be disabled and are ignored there).
/// `Content-Type` and `Content-Length` are listed only when the request has a body; for `Multipart` the value is
/// `multipart/form-data` without the boundary and `Content-Length` is not listed — the exact values are known
/// only after sending (`HttpResponse.sent.headers`).
pub fn auto_headers(request: &HttpRequest, user_agent: &str) -> Vec<(String, String)>;
```

**Правила:**
- TLS: `rustls` с провайдером `ring`, устанавливается один раз до первого клиента (как в
  `spike/http-probe/src/lib.rs`). Авто-редирект и авто-распаковка `reqwest` выключены. Клиенты кэшируются
  внутри экземпляра `ReqwestEngine` по паре (`verify_tls`, `proxy`); глобального кэша нет.
- Автозаголовки: `User-Agent` из `EngineConfig`, `Accept: */*`, `Accept-Encoding: gzip, deflate, br, zstd`,
  `Content-Type` — `default_content_type` либо `multipart/form-data; boundary=…`. `Host`, `Accept` и
  `Content-Length` нельзя отключить, только переопределить: `Host` и `Content-Length` ставит HTTP-стек, а
  `Accept: */*` клиент `reqwest` дописывает всегда — отступление от раздела 7 спеки **[моё решение]**,
  вынесено владельцу.
- Файл тела (`HttpBody::File`, файловые части multipart) открывается до установки соединения; сбой —
  `HttpError::Io`.
- Тело ответа читается потоком и распаковывается потоком в файл `body_dir/<uuid>.body`; память не растёт с
  размером тела. `wire_size` — байты тела «на проводе», `decoded_size` — после распаковки.
- Распаковка по `Content-Encoding` (`gzip`, `x-gzip`, `deflate`, `br`, `zstd`; несколько через запятую — в обратном
  порядке). Пустое тело не распаковывается. Неизвестная кодировка — тело как есть, без ошибки. Сбой распаковки —
  не ошибка запроса: `decode_error = Some(..)`, в файле тело как получено.
- `ttfb` — от начала отправки до прихода заголовков ответа; `total` — до конца тела.
- Отмена и таймаут прерывают и ожидание заголовков, и чтение тела; недописанный файл тела удаляется.
- URL без схемы — ошибка `InvalidUrl` (схему подставляет `rqm-runner`).
- Произвольный метод (`"PURGE"`, `"lower"`) отправляется как есть; пустой или с пробелом → `InvalidUrl`
  не использовать — вернуть `HttpError::Other("invalid method")`.

- [ ] **Step 1: Написать падающие тесты `tests/basic.rs`** (сервер — `wiremock`; помощники в `tests/common/mod.rs`:
  `engine() -> (ReqwestEngine, tempfile::TempDir)` — `user_agent = "RequestMaster/test"`, `body_dir` — временная
  папка; `get(url) -> HttpRequest` — `GET` без заголовков и тела, `SendOptions::default()` с `proxy: ProxySetting::Off`
  (чтобы тесты не зависели от системного прокси машины); `body(resp) -> Vec<u8>`,
  `gzip(bytes)`, `brotli(bytes)`, `zstd(bytes)`, `deflate(bytes)`)

```rust
mod common;
use common::*;
use rqm_http::*;
use tokio_util::sync::CancellationToken;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

#[tokio::test] async fn get_returns_status_headers_body_and_sizes() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/a")).respond_with(ResponseTemplate::new(201).insert_header("x-test", "1").set_body_string("привет")).mount(&server).await;
    let (e, _dir) = engine();
    let r = e.send(get(&format!("{}/a", server.uri())), CancellationToken::new()).await.unwrap();
    assert_eq!(r.status, 201);
    assert!(r.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("x-test") && v == "1"));
    assert_eq!(body(&r), "привет".as_bytes());
    assert_eq!((r.wire_size, r.decoded_size), (12, 12));
    assert!(r.redirects.is_empty() && r.decode_error.is_none());
    assert!(r.timings.ttfb <= r.timings.total);
}
#[tokio::test] async fn automatic_headers_are_sent_and_reported() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(header("user-agent", "RequestMaster/test")).and(header("accept", "*/*"))
        .and(headers("accept-encoding", vec!["gzip", "deflate", "br", "zstd"])).and(header("content-type", "application/json"))
        .and(header("content-length", "2")).and(body_string("{}")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let (e, _dir) = engine();
    let mut req = get(&server.uri()); req.method = "POST".into();
    req.body = HttpBody::Bytes { data: b"{}".to_vec(), default_content_type: Some("application/json".into()) };
    let names: Vec<_> = auto_headers(&req, "RequestMaster/test").into_iter().map(|(k, _)| k).collect();
    assert_eq!(names, vec!["Host", "User-Agent", "Accept", "Accept-Encoding", "Content-Type", "Content-Length"]);
    let r = e.send(req, CancellationToken::new()).await.unwrap();
    assert_eq!(r.status, 200);
    assert!(r.sent.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("user-agent")));
}
#[tokio::test] async fn user_header_overrides_and_disabled_auto_header_is_not_sent() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let (e, _dir) = engine();
    let mut req = get(&server.uri());
    req.headers = vec![("User-Agent".into(), "mine".into()), ("X-Dup".into(), "1".into()), ("X-Dup".into(), "2".into())];
    req.options.disabled_auto_headers = vec!["accept".into(), "accept-encoding".into(), "host".into()];
    let names: Vec<_> = auto_headers(&req, "RequestMaster/test").into_iter().map(|(k, _)| k).collect();
    assert_eq!(names, vec!["Host", "Accept"]);          // these two cannot be disabled; no body = no Content-*
    e.send(req, CancellationToken::new()).await.unwrap();
    let got = &server.received_requests().await.unwrap()[0];
    assert_eq!(got.headers.get("user-agent").unwrap(), "mine");
    assert_eq!(got.headers.get("accept").unwrap(), "*/*");
    assert!(got.headers.get("accept-encoding").is_none());
    assert_eq!(got.headers.get_all("x-dup").iter().count(), 2);
}
#[tokio::test] async fn every_encoding_is_decoded_and_wire_size_is_reported() {
    let plain = "x".repeat(50_000).into_bytes();
    for (name, packed) in [("gzip", gzip(&plain)), ("deflate", deflate(&plain)), ("br", brotli(&plain)), ("zstd", zstd(&plain))] {
        let server = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).insert_header("content-encoding", name).set_body_bytes(packed.clone())).mount(&server).await;
        let (e, _dir) = engine();
        let r = e.send(get(&server.uri()), CancellationToken::new()).await.unwrap();
        assert_eq!(body(&r), plain, "{name}");
        assert_eq!((r.wire_size, r.decoded_size), (packed.len() as u64, 50_000), "{name}");
        assert!(r.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-encoding")), "{name}: header is kept");
    }
}
#[tokio::test] async fn empty_encoded_bodies_are_not_an_error() {
    for (m, status) in [("HEAD", 200), ("GET", 204), ("GET", 304)] {
        let server = MockServer::start().await;
        Mock::given(method(m)).respond_with(ResponseTemplate::new(status).insert_header("content-encoding", "gzip")).mount(&server).await;
        let (e, _dir) = engine();
        let mut req = get(&server.uri()); req.method = m.into();
        let r = e.send(req, CancellationToken::new()).await.unwrap();
        assert_eq!((r.status, r.decoded_size, r.decode_error.is_none()), (status, 0, true), "{m} {status}");
    }
}
#[tokio::test] async fn broken_and_unknown_encodings_keep_the_raw_body() {
    for (enc, expect_error) in [("gzip", true), ("made-up", false)] {
        let server = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).insert_header("content-encoding", enc).set_body_string("not compressed")).mount(&server).await;
        let (e, _dir) = engine();
        let r = e.send(get(&server.uri()), CancellationToken::new()).await.unwrap();
        assert_eq!(body(&r), b"not compressed");
        assert_eq!(r.decode_error.is_some(), expect_error, "{enc}");
    }
}
#[tokio::test] async fn large_body_goes_to_disk() {
    let plain = vec![b'a'; 30 * 1024 * 1024];
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).insert_header("content-encoding", "gzip").set_body_bytes(gzip(&plain))).mount(&server).await;
    let (e, dir) = engine();
    let r = e.send(get(&server.uri()), CancellationToken::new()).await.unwrap();
    assert_eq!(r.decoded_size, 30 * 1024 * 1024);
    assert_eq!(std::fs::metadata(&r.body_path).unwrap().len(), 30 * 1024 * 1024);
    assert!(r.body_path.starts_with(dir.path()));
}
#[tokio::test] async fn ttfb_measures_headers_not_body() {
    // raw TCP server: headers at once, body after 1500 ms
    let addr = slow_body_server(std::time::Duration::from_millis(1500)).await;
    let (e, _dir) = engine();
    let r = e.send(get(&format!("http://{addr}/")), CancellationToken::new()).await.unwrap();
    assert!(r.timings.ttfb < std::time::Duration::from_millis(1000), "ttfb {:?}", r.timings.ttfb);
    assert!(r.timings.total >= std::time::Duration::from_millis(1400), "total {:?}", r.timings.total);
}
#[tokio::test] async fn cancel_and_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(5))).mount(&server).await;
    let (e, dir) = engine();
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move { tokio::time::sleep(std::time::Duration::from_millis(100)).await; c2.cancel(); });
    let started = std::time::Instant::now();
    assert!(matches!(e.send(get(&server.uri()), cancel).await, Err(HttpError::Cancelled)));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    let mut req = get(&server.uri()); req.options.timeout = Some(std::time::Duration::from_millis(150));
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::Timeout)));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "no body files are left behind");
}
#[tokio::test] async fn cancel_during_body_removes_the_partial_file() {
    let addr = slow_body_server(std::time::Duration::from_secs(5)).await;
    let (e, dir) = engine();
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move { tokio::time::sleep(std::time::Duration::from_millis(300)).await; c2.cancel(); });
    assert!(matches!(e.send(get(&format!("http://{addr}/")), cancel).await, Err(HttpError::Cancelled)));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
#[tokio::test] async fn multipart_and_file_bodies() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let (e, dir) = engine();
    let file = dir.path().join("up.bin");
    std::fs::write(&file, b"FILEDATA").unwrap();
    let mut req = get(&server.uri()); req.method = "POST".into();
    req.body = HttpBody::Multipart(vec![
        MultipartPart { name: "text".into(), value: PartValue::Text("значение".into()), content_type: None },
        MultipartPart { name: "f".into(), value: PartValue::File(file.clone()), content_type: Some("application/x-test".into()) }]);
    e.send(req, CancellationToken::new()).await.unwrap();
    let mut req = get(&server.uri()); req.method = "POST".into();
    req.body = HttpBody::File { path: file, default_content_type: Some("application/octet-stream".into()) };
    e.send(req, CancellationToken::new()).await.unwrap();
    let got = server.received_requests().await.unwrap();
    assert!(got[0].headers.get("content-type").unwrap().to_str().unwrap().starts_with("multipart/form-data; boundary="));
    let multipart = String::from_utf8_lossy(&got[0].body);
    assert!(multipart.contains("name=\"text\"") && multipart.contains("значение"));
    assert!(multipart.contains("filename=\"up.bin\"") && multipart.contains("application/x-test") && multipart.contains("FILEDATA"));
    assert_eq!(got[1].body, b"FILEDATA");
    assert_eq!(got[1].headers.get("content-type").unwrap(), "application/octet-stream");
}
#[tokio::test] async fn errors_are_classified() {
    let (e, _dir) = engine();
    assert!(matches!(e.send(get("no-scheme.test/x"), CancellationToken::new()).await, Err(HttpError::InvalidUrl(_))));
    assert!(matches!(e.send(get("http://127.0.0.1:1/"), CancellationToken::new()).await, Err(HttpError::Connect(_))));
    let mut req = get("http://127.0.0.1:1/"); req.method = "BAD METHOD".into();
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::Other(_))));
    let mut req = get("http://127.0.0.1:1/"); req.body = HttpBody::File { path: "Z:/definitely/missing.bin".into(), default_content_type: None };
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::Io(_))));
}
#[tokio::test] async fn custom_method_is_sent_as_is() {
    let server = MockServer::start().await;
    Mock::given(method("PURGE")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let (e, _dir) = engine();
    let mut req = get(&server.uri()); req.method = "PURGE".into();
    assert_eq!(e.send(req, CancellationToken::new()).await.unwrap().status, 200);
}
```

  `slow_body_server(delay)` — помощник в `tests/common/mod.rs`: `tokio::net::TcpListener` на `127.0.0.1:0`;
  на соединение сначала читает запрос до `\r\n\r\n` (иначе Windows сбросит соединение), затем пишет
  `HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n`, ждёт `delay`, пишет `hello`, вызывает
  `shutdown()` и только потом закрывает сокет; возвращает адрес.

- [ ] **Step 2: `cargo test -p rqm-http`** — ошибка компиляции.
- [ ] **Step 3: Реализовать типы, `auto_headers`, `ReqwestEngine` (пока без редиректов: `3xx` возвращается
  как обычный ответ) и потоковую распаковку.** Опереться на `spike/http-probe/src/lib.rs`, но не копировать его
  недостатки (итоги §6, пп. 8, 9, 16, 18).
- [ ] **Step 4: `cargo test -p rqm-http` и clippy** — 13 passed.
- [ ] **Step 5: Проверить дерево зависимостей:** `cargo tree -p rqm-http -i aws-lc-rs`, `-i aws-lc-sys`,
  `-i native-tls`, `-i openssl` — каждая команда должна ответить, что пакета нет.
- [ ] **Step 6: Commit** — «Этап 1A: HTTP-движок — тело, распаковка, время, отмена (rqm-http)».

---

### Task 7: `rqm-http` — редиректы, cookies, прокси, SSL

**Files:**
- Create: `crates/rqm-http/src/redirect.rs`, `src/cookies.rs`, `src/proxy.rs`
- Modify: `crates/rqm-http/src/engine.rs`, `src/lib.rs`, `tests/common/mod.rs`
- Test: `crates/rqm-http/tests/redirects.rs`, `tests/cookies.rs`, `tests/tls_proxy.rs`

**Interfaces:**
- Consumes: Task 6.
- Produces (меняет конструктор движка):

```rust
pub struct Cookie { pub name: String, pub value: String, pub domain: String, pub path: String,
    pub expires_unix: Option<i64>, pub secure: bool, pub http_only: bool }
pub struct CookieJar { /* private, thread-safe */ }
impl CookieJar {
    pub fn new() -> Self;
    /// Unparseable input yields an error; the caller then starts with an empty jar.
    pub fn from_json(json: &str) -> Result<Self, String>;
    /// All unexpired cookies, session cookies included.
    pub fn to_json(&self) -> String;
    pub fn list(&self) -> Vec<Cookie>;
    pub fn clear(&self);
}
impl ReqwestEngine { pub fn new(config: EngineConfig, jar: std::sync::Arc<CookieJar>) -> Result<Self, HttpError>; }
/// Cookies set by the final response (its `Set-Cookie` headers), with domain and path defaulted from
/// `response.sent.url`. Feeds the Cookies tab of the response panel.
pub fn response_cookies(response: &HttpResponse) -> Vec<Cookie>;
/// Pure rule used by the engine; exposed for tests.
pub fn resolve_proxy(setting: &ProxySetting, system: Option<String>) -> Result<Option<String>, HttpError>;
```

  Помощник `engine()` в `tests/common/mod.rs` обновляется: создаёт пустой `CookieJar`; добавить
  `engine_with_jar(jar)`.

**Правила редиректов** (свой цикл; `follow_redirects = false` → `3xx` возвращается как обычный ответ):
- Редирект — только статусы 301, 302, 303, 307, 308 с заголовком `Location`; другой `3xx` или `3xx` без
  `Location` — обычный ответ. Отличия от Postman **[моё решение]**: Postman идёт по любому `3xx` с `Location`
  и добавляет заголовок `Referer`; здесь — список как у браузеров и без `Referer`.
- `Location` разрешается относительно текущего URL; схема не `http`/`https` → обычный ответ.
- 303 → `GET` (для `HEAD` остаётся `HEAD`); 301 и 302 с методом не `GET`/`HEAD` → `GET` (как в Postman по
  умолчанию); 307 и 308 — метод и тело сохраняются (тело `File` и `Multipart` читается заново).
- При смене метода на `GET` тело не отправляется, заголовки `Content-Type`, `Content-Length`,
  `Content-Encoding`, `Transfer-Encoding` снимаются.
- Другой origin (схема, хост или порт отличаются): снимаются `Authorization`, `Cookie`, `Proxy-Authorization`.
  Cookies из jar для нового адреса подставляются по правилам jar.
- Превышение `max_redirects` → `HttpError::TooManyRedirects(пройденные переходы)`; `max_redirects = 0` при
  включённом следовании — первый же редирект даёт эту ошибку.
- `HttpResponse.redirects` — переходы по порядку; `sent` — последний запрос; `ttfb` и `total` — от начала первого.
- Отмена действует в любой момент цепочки.

**Правила cookies:** jar применяется на каждом переходе (и принимает `Set-Cookie` с каждого ответа, включая
ответы-редиректы); домен, путь, `Secure`, срок — по RFC 6265; если у запроса есть свой заголовок `Cookie`,
cookies из jar дописываются к нему через `; ` **[моё решение]**; сессионные cookies сохраняются в `to_json`
наравне с остальными **[моё решение]**.

**Правила прокси и SSL:** `System` — системный прокси Windows (как в пробнике; PAC/WPAD не поддерживается —
итоги §6 п. 6); `Off` — напрямую (в спеке такого варианта нет, добавлен **[моё решение]**: нужен тестам и
пользователю, у которого системный прокси мешает локальным адресам); `Manual(url)` — `http://` или `https://` адрес, иначе `InvalidUrl`.
`verify_tls = false` принимает любой сертификат; `true` — ошибка `HttpError::Tls`.

- [ ] **Step 1: Написать падающие тесты `tests/redirects.rs`**

```rust
mod common;
use common::*;
use rqm_http::*;
use tokio_util::sync::CancellationToken;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

async fn redirecting(status: u16, to: &str) -> MockServer {
    let s = MockServer::start().await;
    Mock::given(path("/start")).respond_with(ResponseTemplate::new(status).insert_header("location", to)).mount(&s).await;
    Mock::given(path("/end")).respond_with(ResponseTemplate::new(200).set_body_string("done")).mount(&s).await;
    s
}
fn post(url: &str) -> HttpRequest {
    let mut r = get(url); r.method = "POST".into();
    r.headers = vec![("Authorization".into(), "Bearer secret".into()), ("Cookie".into(), "a=1".into()), ("X-Keep".into(), "1".into())];
    r.body = HttpBody::Bytes { data: b"payload".to_vec(), default_content_type: Some("text/plain".into()) };
    r
}

#[tokio::test] async fn chain_is_followed_and_reported() {
    let s = redirecting(302, "/end").await;
    let (e, _d) = engine();
    let r = e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
    assert_eq!((r.status, body(&r)), (200, b"done".to_vec()));
    assert_eq!(r.redirects.len(), 1);
    assert_eq!((r.redirects[0].status, r.redirects[0].location.as_str()), (302, "/end"));
    assert!(r.sent.url.ends_with("/end"));
}
#[tokio::test] async fn method_and_body_rules_per_status() {
    for (status, method_after, body_after) in [(301, "GET", ""), (302, "GET", ""), (303, "GET", ""), (307, "POST", "payload"), (308, "POST", "payload")] {
        let s = redirecting(status, "/end").await;
        let (e, _d) = engine();
        e.send(post(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
        let got = s.received_requests().await.unwrap();
        let last = got.iter().find(|r| r.url.path() == "/end").unwrap();
        assert_eq!(last.method.as_str(), method_after, "{status}");
        assert_eq!(String::from_utf8_lossy(&last.body), body_after, "{status}");
        assert_eq!(last.headers.get("content-type").is_some(), !body_after.is_empty(), "{status}: body headers");
        assert_eq!(last.headers.get("x-keep").unwrap(), "1", "{status}");
        assert_eq!(last.headers.get("authorization").unwrap(), "Bearer secret", "{status}: same origin keeps auth");
    }
}
#[tokio::test] async fn head_stays_head_on_303() {
    let s = redirecting(303, "/end").await;
    let (e, _d) = engine();
    let mut req = get(&format!("{}/start", s.uri())); req.method = "HEAD".into();
    e.send(req, CancellationToken::new()).await.unwrap();
    assert_eq!(s.received_requests().await.unwrap().last().unwrap().method.as_str(), "HEAD");
}
#[tokio::test] async fn cross_origin_strips_credentials() {
    let other = MockServer::start().await;
    Mock::given(path("/end")).respond_with(ResponseTemplate::new(200)).mount(&other).await;
    let s = redirecting(307, &format!("{}/end", other.uri())).await;   // another port = another origin
    let (e, _d) = engine();
    e.send(post(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
    let got = &other.received_requests().await.unwrap()[0];
    assert!(got.headers.get("authorization").is_none());
    assert!(got.headers.get("cookie").is_none());
    assert_eq!(got.headers.get("x-keep").unwrap(), "1");
    assert_eq!(got.body, b"payload");
}
#[tokio::test] async fn only_real_redirect_statuses_are_followed() {
    for status in [300, 304, 305, 306] {
        let s = redirecting(status, "/end").await;
        let (e, _d) = engine();
        let r = e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
        assert_eq!((r.status, r.redirects.len()), (status, 0));
    }
    let s = MockServer::start().await;
    Mock::given(path("/start")).respond_with(ResponseTemplate::new(302)).mount(&s).await;   // no Location
    let (e, _d) = engine();
    assert_eq!(e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap().status, 302);
    let s = redirecting(302, "ftp://example.test/x").await;
    let (e, _d) = engine();
    assert_eq!(e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap().status, 302);
}
#[tokio::test] async fn follow_off_and_limit() {
    let s = redirecting(302, "/start").await;                       // endless loop
    let (e, _d) = engine();
    let mut req = get(&format!("{}/start", s.uri())); req.options.follow_redirects = false;
    assert_eq!(e.send(req, CancellationToken::new()).await.unwrap().status, 302);
    let mut req = get(&format!("{}/start", s.uri())); req.options.max_redirects = 3;
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::TooManyRedirects(h)) if h.len() == 3));
    let mut req = get(&format!("{}/start", s.uri())); req.options.max_redirects = 0;
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::TooManyRedirects(_))));
}
#[tokio::test] async fn relative_location_is_resolved() {
    let s = MockServer::start().await;
    Mock::given(path("/a/b/start")).respond_with(ResponseTemplate::new(302).insert_header("location", "../end?x=1")).mount(&s).await;
    Mock::given(path("/a/end")).and(query_param("x", "1")).respond_with(ResponseTemplate::new(200)).mount(&s).await;
    let (e, _d) = engine();
    assert_eq!(e.send(get(&format!("{}/a/b/start", s.uri())), CancellationToken::new()).await.unwrap().status, 200);
}
```

- [ ] **Step 2: Написать падающие тесты `tests/cookies.rs`**

```rust
mod common;
use common::*;
use rqm_http::*;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

#[tokio::test] async fn cookies_are_stored_sent_listed_persisted_and_cleared() {
    let s = MockServer::start().await;
    Mock::given(path("/login")).respond_with(ResponseTemplate::new(200).append_header("set-cookie", "sid=abc; Path=/").append_header("set-cookie", "old=1; Max-Age=0")).mount(&s).await;
    Mock::given(path("/me")).respond_with(ResponseTemplate::new(200)).mount(&s).await;
    let jar = Arc::new(CookieJar::new());
    let (e, _d) = engine_with_jar(jar.clone());
    let first = e.send(get(&format!("{}/login", s.uri())), CancellationToken::new()).await.unwrap();
    let set: Vec<_> = response_cookies(&first).into_iter().map(|c| (c.name, c.path, c.domain)).collect();
    assert_eq!(set, vec![("sid".to_string(), "/".to_string(), "127.0.0.1".to_string()), ("old".to_string(), "/".to_string(), "127.0.0.1".to_string())]);
    assert_eq!(jar.list().iter().map(|c| (c.name.as_str(), c.value.as_str())).collect::<Vec<_>>(), vec![("sid", "abc")]);
    e.send(get(&format!("{}/me", s.uri())), CancellationToken::new()).await.unwrap();
    assert_eq!(s.received_requests().await.unwrap()[1].headers.get("cookie").unwrap(), "sid=abc");

    let restored = Arc::new(CookieJar::from_json(&jar.to_json()).unwrap());   // session cookie survives a restart
    let (e2, _d2) = engine_with_jar(restored.clone());
    let mut req = get(&format!("{}/me", s.uri())); req.headers = vec![("Cookie".into(), "mine=1".into())];
    e2.send(req, CancellationToken::new()).await.unwrap();
    assert_eq!(s.received_requests().await.unwrap()[2].headers.get("cookie").unwrap(), "mine=1; sid=abc");

    restored.clear();
    assert!(restored.list().is_empty());
    assert!(CookieJar::from_json("garbage").is_err());
}
#[tokio::test] async fn cookie_set_on_a_redirect_is_sent_to_the_next_hop() {
    let s = MockServer::start().await;
    Mock::given(path("/start")).respond_with(ResponseTemplate::new(302).insert_header("location", "/end").insert_header("set-cookie", "hop=1; Path=/")).mount(&s).await;
    Mock::given(path("/end")).respond_with(ResponseTemplate::new(200)).mount(&s).await;
    let (e, _d) = engine();
    e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
    assert_eq!(s.received_requests().await.unwrap()[1].headers.get("cookie").unwrap(), "hop=1");
}
#[tokio::test] async fn cookies_do_not_leak_to_another_host_or_path() {
    let a = MockServer::start().await;
    Mock::given(path("/set")).respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "p=1; Path=/only")).mount(&a).await;
    Mock::given(any()).respond_with(ResponseTemplate::new(200)).mount(&a).await;
    let (e, _d) = engine();
    e.send(get(&format!("{}/set", a.uri())), CancellationToken::new()).await.unwrap();
    e.send(get(&format!("{}/other", a.uri())), CancellationToken::new()).await.unwrap();
    e.send(get(&format!("{}/only/x", a.uri())), CancellationToken::new()).await.unwrap();
    let got = a.received_requests().await.unwrap();
    assert!(got[1].headers.get("cookie").is_none());
    assert_eq!(got[2].headers.get("cookie").unwrap(), "p=1");
}
```

- [ ] **Step 3: Написать падающие тесты `tests/tls_proxy.rs`** — сервер HTTPS с самоподписанным сертификатом
  (`rcgen` + `tokio-rustls`, помощник `https_server() -> SocketAddr` в `tests/common/mod.rs`, как в
  `spike/http-probe/tests`; сертификат — `generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])`,
  сервер отвечает `200`):

```rust
mod common;
use common::*;
use rqm_http::*;
use tokio_util::sync::CancellationToken;

#[tokio::test] async fn self_signed_certificate_needs_verify_off() {
    let addr = https_server().await;
    let (e, _d) = engine();
    let url = format!("https://127.0.0.1:{}/", addr.port());
    assert!(matches!(e.send(get(&url), CancellationToken::new()).await, Err(HttpError::Tls(_))));
    let mut req = get(&url); req.options.verify_tls = false;
    assert_eq!(e.send(req, CancellationToken::new()).await.unwrap().status, 200);
    // the strict client is still strict after a relaxed one was used
    assert!(matches!(e.send(get(&url), CancellationToken::new()).await, Err(HttpError::Tls(_))));
}
#[tokio::test] async fn manual_proxy_receives_the_request() {
    let proxy = wiremock::MockServer::start().await;                 // acts as a plain-HTTP proxy
    wiremock::Mock::given(wiremock::matchers::any()).respond_with(wiremock::ResponseTemplate::new(299)).mount(&proxy).await;
    let (e, _d) = engine();
    let mut req = get("http://target.invalid/x"); req.options.proxy = ProxySetting::Manual(proxy.uri());
    assert_eq!(e.send(req, CancellationToken::new()).await.unwrap().status, 299);
    let mut req = get("http://target.invalid/x"); req.options.proxy = ProxySetting::Manual("socks9://nope".into());
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::InvalidUrl(_))));
}
#[test] fn proxy_resolution_rules() {
    assert_eq!(resolve_proxy(&ProxySetting::Off, Some("http://sys:1".into())).unwrap(), None);
    assert_eq!(resolve_proxy(&ProxySetting::System, Some("http://sys:1".into())).unwrap(), Some("http://sys:1".into()));
    assert_eq!(resolve_proxy(&ProxySetting::System, None).unwrap(), None);
    assert_eq!(resolve_proxy(&ProxySetting::Manual("http://m:2".into()), Some("http://sys:1".into())).unwrap(), Some("http://m:2".into()));
    assert!(resolve_proxy(&ProxySetting::Manual("ftp://m".into()), None).is_err());
}
```

- [ ] **Step 4: `cargo test -p rqm-http`** — ошибка компиляции.
- [ ] **Step 5: Реализовать цикл редиректов, `CookieJar`, прокси и переключатель SSL.**
- [ ] **Step 6: `cargo test -p rqm-http` и clippy** — 13 + 7 + 3 + 3 passed.
- [ ] **Step 7: Commit** — «Этап 1A: редиректы, cookies, прокси и SSL (rqm-http)».

---

### Task 8: `rqm-runner` — сценарий «Send» без скриптов

**Files:**
- Create: `crates/rqm-runner/Cargo.toml`, `src/lib.rs`, `src/prepare.rs`, `src/auth.rs`, `src/send.rs`,
  `src/body_view.rs`, `src/cookies_state.rs`
- Test: `crates/rqm-runner/tests/prepare.rs`, `crates/rqm-runner/tests/send.rs`

**Interfaces:**
- Consumes: `rqm-model` (Task 1), `rqm-vars` (Task 4), `rqm-store` (Task 5), `rqm-http` (Task 6–7).
- Produces:

```rust
pub struct AppSettings { pub timeout: Option<std::time::Duration>, pub verify_tls: bool,
                         pub proxy: rqm_http::ProxySetting }
impl Default for AppSettings   // no timeout, verify TLS, system proxy
/// Inline display limit from the spec, section 4: 20 MiB.
pub const INLINE_BODY_LIMIT: u64 = 20 * 1024 * 1024;
/// How much of a larger body is shown: 256 KiB.
pub const TRUNCATED_PREVIEW: u64 = 256 * 1024;

pub struct SendInput { pub request_name: String, pub request: rqm_model::RequestContent,
    /// From the collection down to the direct parent folder.
    pub ancestors: Vec<rqm_model::Node>,
    pub environment: Option<rqm_model::Environment>, pub globals: rqm_model::Environment,
    pub settings: AppSettings }
/// Reads the request (or uses `draft` instead of the stored content), its ancestors, globals and the environment.
/// A node that is not a request, or an unknown environment id, yields `StoreError::NotFound`.
pub fn load_send_input(store: &rqm_store::Store, request_id: &str, draft: Option<rqm_model::RequestContent>,
    environment_id: Option<&str>, settings: AppSettings) -> Result<SendInput, rqm_store::StoreError>;

pub enum SendWarning { ScriptsNotExecuted { nodes: Vec<String> }, UnsupportedAuth { auth_type: String },
    BodyPruned { method: String }, UnresolvedVariables { names: Vec<String> } }
pub enum PrepareError { UnconfirmedFiles { fields: Vec<String> }, InvalidUrl(String) }
pub struct Prepared { pub http: rqm_http::HttpRequest, pub warnings: Vec<SendWarning> }
/// Pure: no network, no disk. Uses `dynamic` for `{{$...}}`.
pub fn prepare(input: &SendInput, dynamic: &mut dyn FnMut(&str) -> Option<String>) -> Result<Prepared, PrepareError>;

pub enum SendError { Prepare(PrepareError), Http(rqm_http::HttpError) }
pub struct SendOutcome { pub result: Result<rqm_http::HttpResponse, SendError>, pub warnings: Vec<SendWarning> }
pub async fn send(engine: &dyn rqm_http::HttpEngine, input: &SendInput,
                  cancel: tokio_util::sync::CancellationToken) -> SendOutcome;

pub struct BodyView { pub bytes: Vec<u8>, pub total: u64, pub truncated: bool, pub pretty: bool }
/// Whole body when `total <= INLINE_BODY_LIMIT`, otherwise the first `TRUNCATED_PREVIEW` bytes.
/// `pretty = true` re-indents JSON (2 spaces, key order kept) when the body is valid JSON and not truncated.
pub fn body_view(path: &std::path::Path, pretty: bool) -> std::io::Result<BodyView>;

pub const COOKIE_JAR_KEY: &str = "cookie_jar";
/// Empty jar when nothing is stored or the stored value is unreadable.
pub fn load_cookies(store: &rqm_store::Store) -> Result<rqm_http::CookieJar, rqm_store::StoreError>;
pub fn save_cookies(store: &mut rqm_store::Store, jar: &rqm_http::CookieJar) -> Result<(), rqm_store::StoreError>;
```

**Правила `prepare`** (спека, разделы 4, 5, 7, 9):
1. Области: globals < переменные коллекции < окружение; только включённые переменные; local на этапе 1 пуст.
2. Подстановка — в: URL, ключи и значения query, значения path-переменных, ключи и значения заголовков,
   тело (`Raw.text`, `UrlEncoded`, текстовые поля `FormData`, `GraphQl.query` и `variables`), все поля auth.
   В пути и содержимое файлов — нет. Все неразрешённые имена → одно предупреждение `UnresolvedVariables`
   (без повторов, по порядку появления); отправку оно не блокирует.
3. URL: подстановка → path-переменные (включённые) → если нет схемы (`://`), дописать `http://` → добавить
   включённые query-параметры (`key=value` через `&`; percent-кодируются только символы, недопустимые в query:
   пробел, `"`, `#`, `<`, `>`, `&` в значении, `=` в ключе). Фрагмент `#…` из адреса отбрасывается. URL, пустой
   после подстановки и обрезки пробелов по краям → `PrepareError::InvalidUrl`.
4. Заголовки: включённые, по порядку, с пустым ключом — пропускаются.
5. Auth: свой `auth` запроса; если `None` — ближайший предок с `Some(..)` (от папки к коллекции); не найден →
   без авторизации. `NoAuth` останавливает подъём. `Basic` → `Authorization: Basic base64(user:password)`;
   `Bearer` → `Authorization: Bearer <token>`; `OAuth2Token` с `placement = Header` → `Authorization: <prefix> <token>`
   (при пустом префиксе — один токен), с `placement = Query` → query-параметр `access_token=<token>` (одноимённый
   параметр запроса заменяется); `ApiKey` → заголовок или query-параметр; `Unsupported` → ничего + предупреждение
   `UnsupportedAuth`. Заголовок от auth заменяет одноимённый заголовок запроса (как в Postman, сверено с
   postman-runtime). Пустые значения — как в Postman: `Bearer`/`OAuth2Token` с пустым токеном — ничего не
   добавлять; `Basic` добавляется всегда (пустые логин и пароль → `Basic Og==`); `ApiKey` пропускается только
   при пустом ключе, пустое значение при непустом ключе отправляется.
6. Тело: `Raw` → `Bytes` с типом по языку (`Json` → `application/json`, `Xml` → `application/xml`, `Html` →
   `text/html`, `JavaScript` → `application/javascript`, `Text` → `text/plain`); `UrlEncoded` → включённые пары
   в `application/x-www-form-urlencoded`; `FormData` → `Multipart` из включённых частей (несколько файлов в одном
   поле — несколько частей с одним именем); `Binary` → `File` без типа по умолчанию (как в Postman); `GraphQl` →
   `{"query": <JSON-строка>, "variables": <текст variables>}` с `application/json`: валидный JSON в `variables`
   пишется компактно, пустая или состоящая из пробелов строка — ключ не пишется, невалидный JSON вставляется
   в тело как есть (как в Postman; тело при этом невалидно); `None` → `HttpBody::None`.
7. Файлы: любой `FileRef { confirmed: false }` во включённой части или в `Binary` →
   `PrepareError::UnconfirmedFiles` с именами полей (`"binary"` для `Binary`); запрос не отправляется
   **[моё решение]** (спека: «не отправляется, пока пользователь не выберет его заново»). `Binary { file: None }` →
   тело пустое.
8. Обрезка тела: метод (без учёта регистра) `GET`, `HEAD`, `COPY`, `PURGE` или `UNLOCK`, тело не `None` и
   `disable_body_pruning != Some(true)` → тело не отправляется + предупреждение `BodyPruned` (список методов
   сверен с postman-runtime).
9. Настройки: `follow_redirects` — запроса, иначе `true`; `max_redirects` — запроса, иначе `10`; `verify_tls` —
   `strict_ssl` запроса, иначе общая; `timeout`, `proxy` — общие; `disabled_auto_headers` — из запроса.
10. Скрипты: если непустой скрипт есть у запроса или у любого предка — предупреждение `ScriptsNotExecuted`
    с именами этих узлов от коллекции к запросу. Отправку не блокирует.

`send` = `prepare` с `rqm_vars::system_dynamic` → `engine.send`. Предупреждения возвращаются и при ошибке сети.

- [ ] **Step 1: Написать падающие тесты `tests/prepare.rs`**

```rust
use rqm_http::{HttpBody, PartValue};
use rqm_model::*;
use rqm_runner::*;

fn p(k: &str, v: &str) -> Param { Param { key: k.into(), value: v.into(), enabled: true, postman_extra: Extra::new() } }
fn off(k: &str, v: &str) -> Param { Param { enabled: false, ..p(k, v) } }
fn var(k: &str, v: &str) -> Variable { Variable { key: k.into(), value: v.into(), secret: false, enabled: true, postman_extra: Extra::new() } }
fn base_request() -> RequestContent {
    RequestContent { method: "GET".into(), url: "https://h.test/x".into(), query: vec![], path_variables: vec![], headers: vec![],
        body: Body::None, auth: None, scripts: Scripts::default(), settings: RequestSettings::default(), disabled_auto_headers: vec![], postman_extra: Extra::new() }
}
fn collection(auth: Option<Auth>, vars: Vec<Variable>, pre: &str) -> Node {
    Node { id: new_id(), parent_id: None, kind: NodeKind::Collection, name: "Col".into(), sort_order: 0,
        content: NodeContent::Collection(CollectionContent { auth, variables: vars, scripts: Scripts { pre_request: pre.into(), test: String::new() }, postman_extra: Extra::new() }) }
}
fn folder(name: &str, auth: Option<Auth>) -> Node {
    Node { id: new_id(), parent_id: Some("x".into()), kind: NodeKind::Folder, name: name.into(), sort_order: 0,
        content: NodeContent::Folder(FolderContent { auth, scripts: Scripts::default(), postman_extra: Extra::new() }) }
}
fn input(request: RequestContent, ancestors: Vec<Node>) -> SendInput {
    SendInput { request_name: "Req".into(), request, ancestors, environment: None, globals: Environment::empty_globals(), settings: AppSettings::default() }
}
fn prep(i: &SendInput) -> Prepared { prepare(i, &mut |_| None).unwrap() }
fn header<'a>(h: &'a Prepared, name: &str) -> Option<&'a str> { h.http.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()) }
fn bytes(h: &Prepared) -> (String, Option<String>) { match &h.http.body { HttpBody::Bytes { data, default_content_type } => (String::from_utf8(data.clone()).unwrap(), default_content_type.clone()), _ => panic!("not bytes") } }

#[test] fn scope_priority_and_substitution_everywhere() {
    let mut r = base_request();
    r.method = "POST".into();
    r.url = "{{base}}/u/:id".into();
    r.path_variables = vec![p("id", "{{who}}")];
    r.query = vec![p("{{k}}", "{{who}} x&y"), off("skip", "1")];
    r.headers = vec![p("X-{{k}}", "{{who}}"), off("X-Off", "1"), p("", "ignored")];
    r.body = Body::Raw { text: "{\"a\":\"{{who}}\",\"b\":\"{{nope}}\"}".into(), language: RawLanguage::Json };
    let mut i = input(r, vec![collection(None, vec![var("who", "collection"), var("k", "key")], "")]);
    i.globals.variables = vec![var("who", "global"), var("base", "h.test")];
    i.environment = Some(Environment { id: new_id(), kind: EnvKind::Environment, name: "E".into(), variables: vec![var("who", "env"), Variable { enabled: false, ..var("k", "disabled") }], postman_extra: Extra::new() });
    let out = prep(&i);
    assert_eq!(out.http.url, "http://h.test/u/env?key=env%20x%26y");
    assert_eq!(header(&out, "X-key"), Some("env"));
    assert_eq!(out.http.headers.len(), 1);
    assert_eq!(bytes(&out), ("{\"a\":\"env\",\"b\":\"{{nope}}\"}".into(), Some("application/json".into())));
    assert!(matches!(&out.warnings[..], [SendWarning::UnresolvedVariables { names }] if names == &vec!["nope".to_string()]));
}
#[test] fn auth_is_inherited_from_the_nearest_ancestor() {
    let col = collection(Some(Auth::Bearer { token: "{{t}}".into() }), vec![var("t", "tok")], "");
    assert_eq!(header(&prep(&input(base_request(), vec![col.clone(), folder("F", None)])), "authorization"), Some("Bearer tok"));
    let basic = folder("F", Some(Auth::Basic { username: "u".into(), password: "p".into() }));
    assert_eq!(header(&prep(&input(base_request(), vec![col.clone(), basic])), "authorization"), Some("Basic dTpw"));
    assert_eq!(header(&prep(&input(base_request(), vec![col.clone(), folder("F", Some(Auth::NoAuth))])), "authorization"), None);
    let mut r = base_request(); r.auth = Some(Auth::NoAuth);
    assert_eq!(header(&prep(&input(r, vec![col.clone()])), "authorization"), None);
    assert_eq!(header(&prep(&input(base_request(), vec![])), "authorization"), None);
}
#[test] fn auth_kinds() {
    let with = |a: Auth| { let mut r = base_request(); r.auth = Some(a); r.headers = vec![p("Authorization", "manual")]; prep(&input(r, vec![])) };
    let oauth = |prefix: &str, placement| Auth::OAuth2Token { token: "t".into(), header_prefix: prefix.into(), placement };
    assert_eq!(header(&with(oauth("Token", ApiKeyPlacement::Header)), "authorization"), Some("Token t"));
    assert_eq!(header(&with(oauth("", ApiKeyPlacement::Header)), "authorization"), Some("t"));
    let oq = with(oauth("Bearer", ApiKeyPlacement::Query));
    assert_eq!((oq.http.url.as_str(), header(&oq, "authorization")), ("https://h.test/x?access_token=t", Some("manual")));
    assert_eq!(header(&with(Auth::Basic { username: String::new(), password: String::new() }), "authorization"), Some("Basic Og=="));
    assert_eq!(header(&with(Auth::ApiKey { key: String::new(), value: "v".into(), placement: ApiKeyPlacement::Header }), "authorization"), Some("manual"));
    assert_eq!(header(&with(Auth::Bearer { token: "t".into() }), "authorization"), Some("Bearer t"));
    assert_eq!(header(&with(Auth::Bearer { token: String::new() }), "authorization"), Some("manual"));
    let k = with(Auth::ApiKey { key: "X-Api".into(), value: "v".into(), placement: ApiKeyPlacement::Header });
    assert_eq!(header(&k, "x-api"), Some("v"));
    let q = with(Auth::ApiKey { key: "api_key".into(), value: "v".into(), placement: ApiKeyPlacement::Query });
    assert_eq!(q.http.url, "https://h.test/x?api_key=v");
    let u = with(Auth::Unsupported { auth_type: "awsv4".into() });
    assert_eq!(header(&u, "authorization"), Some("manual"));
    assert!(u.warnings.iter().any(|w| matches!(w, SendWarning::UnsupportedAuth { auth_type } if auth_type == "awsv4")));
}
#[test] fn bodies() {
    let with = |b: Body| { let mut r = base_request(); r.method = "POST".into(); r.body = b; prep(&input(r, vec![])) };
    for (lang, ct) in [(RawLanguage::Text, "text/plain"), (RawLanguage::Xml, "application/xml"), (RawLanguage::Html, "text/html"), (RawLanguage::JavaScript, "application/javascript")] {
        assert_eq!(bytes(&with(Body::Raw { text: "t".into(), language: lang })).1.as_deref(), Some(ct));
    }
    assert_eq!(bytes(&with(Body::UrlEncoded(vec![p("a b", "1&2"), off("x", "y"), p("я", "ю")]))),
        ("a+b=1%262&%D1%8F=%D1%8E".into(), Some("application/x-www-form-urlencoded".into())));
    assert_eq!(bytes(&with(Body::GraphQl { query: "{ a }".into(), variables: "{\"n\": 1}".into() })),
        ("{\"query\":\"{ a }\",\"variables\":{\"n\":1}}".into(), Some("application/json".into())));
    assert_eq!(bytes(&with(Body::GraphQl { query: "{ a }".into(), variables: "  ".into() })).0, "{\"query\":\"{ a }\"}");
    assert_eq!(bytes(&with(Body::GraphQl { query: "q".into(), variables: "{oops".into() })).0, "{\"query\":\"q\",\"variables\":{oops}");
    assert!(matches!(with(Body::Binary { file: None }).http.body, HttpBody::None));
    assert!(matches!(with(Body::Binary { file: Some(FileRef { path: "C:/f.bin".into(), confirmed: true }) }).http.body, HttpBody::File { default_content_type: None, .. }));
    let form = with(Body::FormData(vec![
        FormPart { key: "t".into(), value: FormValue::Text("v".into()), enabled: true, content_type: None, postman_extra: Extra::new() },
        FormPart { key: "f".into(), value: FormValue::Files(vec![FileRef { path: "C:/1".into(), confirmed: true }, FileRef { path: "C:/2".into(), confirmed: true }]), enabled: true, content_type: Some("x/y".into()), postman_extra: Extra::new() },
        FormPart { key: "skipped".into(), value: FormValue::Files(vec![FileRef { path: "C:/3".into(), confirmed: false }]), enabled: false, content_type: None, postman_extra: Extra::new() }]));
    let HttpBody::Multipart(parts) = &form.http.body else { panic!() };
    assert_eq!(parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["t", "f", "f"]);
    assert!(matches!(parts[1].value, PartValue::File(_)) && parts[1].content_type.as_deref() == Some("x/y"));
}
#[test] fn unconfirmed_files_block_the_send() {
    let mut r = base_request(); r.method = "POST".into();
    r.body = Body::FormData(vec![FormPart { key: "doc".into(), value: FormValue::Files(vec![FileRef { path: "C:/secret.txt".into(), confirmed: false }]), enabled: true, content_type: None, postman_extra: Extra::new() }]);
    assert!(matches!(prepare(&input(r, vec![]), &mut |_| None), Err(PrepareError::UnconfirmedFiles { fields }) if fields == vec!["doc".to_string()]));
    let mut r = base_request(); r.method = "POST".into();
    r.body = Body::Binary { file: Some(FileRef { path: "C:/f".into(), confirmed: false }) };
    assert!(matches!(prepare(&input(r, vec![]), &mut |_| None), Err(PrepareError::UnconfirmedFiles { fields }) if fields == vec!["binary".to_string()]));
}
#[test] fn body_pruning() {
    for (m, pruned) in [("GET", true), ("get", true), ("HEAD", true), ("COPY", true), ("PURGE", true), ("UNLOCK", true), ("POST", false), ("DELETE", false)] {
        let mut r = base_request(); r.method = m.into(); r.body = Body::Raw { text: "x".into(), language: RawLanguage::Text };
        let out = prep(&input(r, vec![]));
        assert_eq!(matches!(out.http.body, HttpBody::None), pruned, "{m}");
        assert_eq!(out.warnings.iter().any(|w| matches!(w, SendWarning::BodyPruned { .. })), pruned, "{m}");
    }
    let mut r = base_request(); r.body = Body::Raw { text: "x".into(), language: RawLanguage::Text };
    r.settings.disable_body_pruning = Some(true);
    let out = prep(&input(r, vec![]));
    assert!(matches!(out.http.body, HttpBody::Bytes { .. }) && out.warnings.is_empty());
    assert!(prep(&input(base_request(), vec![])).warnings.is_empty());          // GET without a body: no warning
}
#[test] fn settings_resolution() {
    let mut i = input(base_request(), vec![]);
    i.settings.verify_tls = false;
    i.settings.timeout = Some(std::time::Duration::from_secs(3));
    i.request.disabled_auto_headers = vec!["accept-encoding".into()];
    let o = prep(&i).http.options;
    assert_eq!((o.follow_redirects, o.max_redirects, o.verify_tls, o.timeout), (true, 10, false, Some(std::time::Duration::from_secs(3))));
    assert_eq!(o.disabled_auto_headers, vec!["accept-encoding".to_string()]);
    i.request.settings = RequestSettings { follow_redirects: Some(false), max_redirects: Some(2), strict_ssl: Some(true), disable_body_pruning: None };
    let o = prep(&i).http.options;
    assert_eq!((o.follow_redirects, o.max_redirects, o.verify_tls), (false, 2, true));
}
#[test] fn scripts_produce_a_warning_naming_the_nodes() {
    let mut r = base_request(); r.scripts.test = "pm.test('x', () => {})".into();
    let out = prep(&input(r, vec![collection(None, vec![], "console.log(1)"), folder("F", None)]));
    assert!(matches!(&out.warnings[..], [SendWarning::ScriptsNotExecuted { nodes }] if nodes == &vec!["Col".to_string(), "Req".to_string()]));
}
#[test] fn url_rules() {
    let mut r = base_request(); r.url = "h.test/a b#frag".into(); r.query = vec![p("q", "")];
    assert_eq!(prep(&input(r, vec![])).http.url, "http://h.test/a b?q=");
    let mut r = base_request(); r.url = "  ".into();
    assert!(matches!(prepare(&input(r, vec![]), &mut |_| None), Err(PrepareError::InvalidUrl(_))));
    let mut r = base_request(); r.url = "https://h.test/{{$guid}}/{{$guid}}".into();
    let mut n = 0;
    let out = prepare(&input(r, vec![]), &mut |name| { n += 1; (name == "guid").then(|| n.to_string()) }).unwrap();
    assert_eq!(out.http.url, "https://h.test/1/2");
}
```

- [ ] **Step 2: Написать падающие тесты `tests/send.rs`**

```rust
use rqm_http::{CookieJar, EngineConfig, ReqwestEngine};
use rqm_model::*;
use rqm_runner::*;
use rqm_store::Store;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

fn engine(jar: Arc<CookieJar>) -> (ReqwestEngine, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    (ReqwestEngine::new(EngineConfig { body_dir: dir.path().to_path_buf(), user_agent: "RequestMaster/test".into() }, jar).unwrap(), dir)
}
fn request(url: &str) -> RequestContent {
    RequestContent { method: "POST".into(), url: url.into(), query: vec![], path_variables: vec![],
        headers: vec![], body: Body::Raw { text: "{\"n\":{{n}}}".into(), language: RawLanguage::Json }, auth: None,
        scripts: Scripts::default(), settings: RequestSettings::default(), disabled_auto_headers: vec![], postman_extra: Extra::new() }
}
fn settings() -> AppSettings { AppSettings { proxy: rqm_http::ProxySetting::Off, ..Default::default() } }   // independent of the machine's proxy

#[tokio::test] async fn stored_request_is_sent_with_inherited_auth_and_variables() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/items")).and(header("authorization", "Bearer tok")).and(body_string("{\"n\":5}"))
        .respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "sid=1; Path=/").set_body_string("{\"ok\":true,\"list\":[1,2]}")).mount(&server).await;
    let mut store = Store::open_in_memory().unwrap();
    let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(CollectionContent {
        auth: Some(Auth::Bearer { token: "{{token}}".into() }),
        variables: vec![Variable { key: "token".into(), value: "tok".into(), secret: true, enabled: true, postman_extra: Extra::new() }],
        scripts: Scripts::default(), postman_extra: Extra::new() })).unwrap();
    let id = store.create_node(Some(&col), NodeKind::Request, "Create", &NodeContent::Request(request("{{base}}/items"))).unwrap();
    let env = Environment { id: new_id(), kind: EnvKind::Environment, name: "Local".into(), postman_extra: Extra::new(),
        variables: vec![Variable { key: "base".into(), value: server.uri(), secret: false, enabled: true, postman_extra: Extra::new() },
                        Variable { key: "n".into(), value: "5".into(), secret: false, enabled: true, postman_extra: Extra::new() }] };
    store.save_environment(&env).unwrap();

    let jar = Arc::new(load_cookies(&store).unwrap());
    let (e, _dir) = engine(jar.clone());
    let input = load_send_input(&store, &id, None, Some(&env.id), settings()).unwrap();
    let out = send(&e, &input, CancellationToken::new()).await;
    let resp = out.result.unwrap();
    assert_eq!(resp.status, 200);
    assert!(out.warnings.is_empty());

    let view = body_view(&resp.body_path, true).unwrap();
    assert_eq!(String::from_utf8(view.bytes).unwrap(), "{\n  \"ok\": true,\n  \"list\": [\n    1,\n    2\n  ]\n}");
    assert!(view.pretty && !view.truncated);
    assert_eq!(body_view(&resp.body_path, false).unwrap().bytes, b"{\"ok\":true,\"list\":[1,2]}");

    save_cookies(&mut store, &jar).unwrap();
    assert_eq!(load_cookies(&store).unwrap().list().len(), 1);
}
#[tokio::test] async fn draft_is_sent_instead_of_the_stored_content() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/draft")).respond_with(ResponseTemplate::new(204)).mount(&server).await;
    let mut store = Store::open_in_memory().unwrap();
    let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(CollectionContent { auth: None, variables: vec![], scripts: Scripts::default(), postman_extra: Extra::new() })).unwrap();
    let id = store.create_node(Some(&col), NodeKind::Request, "R", &NodeContent::Request(request("http://127.0.0.1:1/stored"))).unwrap();
    let mut draft = request(&format!("{}/draft", server.uri()));
    draft.body = Body::None;
    let (e, _dir) = engine(Arc::new(CookieJar::new()));
    let input = load_send_input(&store, &id, Some(draft), None, settings()).unwrap();
    assert_eq!(send(&e, &input, CancellationToken::new()).await.result.unwrap().status, 204);
}
#[tokio::test] async fn warnings_survive_a_network_error_and_prepare_errors_are_reported() {
    let (e, _dir) = engine(Arc::new(CookieJar::new()));
    let mut r = request("http://127.0.0.1:1/x"); r.scripts.pre_request = "x".into(); r.body = Body::None;
    let input = SendInput { request_name: "R".into(), request: r, ancestors: vec![], environment: None, globals: Environment::empty_globals(), settings: settings() };
    let out = send(&e, &input, CancellationToken::new()).await;
    assert!(matches!(out.result, Err(SendError::Http(rqm_http::HttpError::Connect(_)))));
    assert!(matches!(&out.warnings[..], [SendWarning::ScriptsNotExecuted { .. }]));
    let mut r = request(""); r.body = Body::None;
    let input = SendInput { request_name: "R".into(), request: r, ancestors: vec![], environment: None, globals: Environment::empty_globals(), settings: settings() };
    assert!(matches!(send(&e, &input, CancellationToken::new()).await.result, Err(SendError::Prepare(PrepareError::InvalidUrl(_)))));
}
#[test] fn load_send_input_errors_and_cookie_fallback() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(load_send_input(&store, "missing", None, None, settings()).is_err());
    let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(CollectionContent { auth: None, variables: vec![], scripts: Scripts::default(), postman_extra: Extra::new() })).unwrap();
    assert!(load_send_input(&store, &col, None, None, settings()).is_err(), "a collection is not a request");
    store.state_set(COOKIE_JAR_KEY, "garbage").unwrap();
    assert!(load_cookies(&store).unwrap().list().is_empty());
}
#[test] fn body_view_limits() {
    let dir = tempfile::tempdir().unwrap();
    let small = dir.path().join("s"); std::fs::write(&small, b"not json").unwrap();
    let v = body_view(&small, true).unwrap();
    assert_eq!((v.bytes.as_slice(), v.pretty, v.truncated, v.total), (&b"not json"[..], false, false, 8));
    let big = dir.path().join("b");
    let f = std::fs::File::create(&big).unwrap(); f.set_len(INLINE_BODY_LIMIT + 1).unwrap();
    let v = body_view(&big, true).unwrap();
    assert_eq!((v.bytes.len() as u64, v.truncated, v.pretty, v.total), (TRUNCATED_PREVIEW, true, false, INLINE_BODY_LIMIT + 1));
    let edge = dir.path().join("e");
    let f = std::fs::File::create(&edge).unwrap(); f.set_len(INLINE_BODY_LIMIT).unwrap();
    assert!(!body_view(&edge, false).unwrap().truncated);
}
```

- [ ] **Step 3: `cargo test -p rqm-runner`** — ошибка компиляции.
- [ ] **Step 4: Реализовать `prepare`, `send`, `load_send_input`, `body_view`, `load_cookies`, `save_cookies`.**
- [ ] **Step 5: `cargo test -p rqm-runner` и clippy** — 9 + 5 passed.
- [ ] **Step 6: Commit** — «Этап 1A: сценарий отправки запроса без скриптов (rqm-runner)».

---

### Task 9: Сквозная проверка на образце владельца, чистка пробников, документы

**Files:**
- Create: `crates/rqm-runner/tests/owner_end_to_end.rs`
- Modify: `crates/rqm-runner/Cargo.toml` — `[dev-dependencies] rqm-postman = { path = "../rqm-postman" }` (только
  для этого теста; в обычных зависимостях `rqm-runner` его нет)
- Delete: `spike/postman-probe/`, `spike/http-probe/` (их знания перенесены в `rqm-postman` и `rqm-http`)
  **[моё решение]** — ответ на открытый вопрос итогов §8 п. 1, подтверждается владельцем вместе с планом;
  `spike/script-probe/` и `spike/shell/` остаются — они нужны этапу 2 и плану 1B
- Modify: `spike/README.md` (что осталось и почему)
- Modify: `docs/design.md` — раздел 5 (механизм P1–P4 вместо «мешка на
  каждом уровне»; `disableBodyPruning` в списке применяемых ключей; пробел с `null` закрыт), раздел 5 (в таблице `environment` нет
  отдельного вида), раздел 7 (`Host`, `Accept` и `Content-Length` не отключаются; отключение автозаголовков
  хранится в запросе и переносится в Postman как `disabledSystemHeaders`; обрезка тела у
  `GET`/`HEAD`/`COPY`/`PURGE`/`UNLOCK`; вариант прокси «без прокси»; OAuth 2.0 — токен в заголовке или в query),
  раздел 12
  (этап 1 разбит на 1A/1B/1C; 1A выполнен)
- Create: `stage1a-results.md` — что сделано, числа тестов, список всех
  **[проверить]** и **[моё решение]** этого плана с тем, как каждое реализовано, перечень отличий от Postman
  (статусы редиректа, нет `Referer`, HTTP/2, PAC/WPAD, неотключаемые автозаголовки) и что осталось открытым

**Interfaces:**
- Consumes: все crates.
- Produces: ничего нового.

- [ ] **Step 1: Написать тест `crates/rqm-runner/tests/owner_end_to_end.rs`**

```rust
/// Owner collections: import → store → read back → export with an empty diff; every request can be prepared.
/// Prints counts only — never names, URLs or values from the owner's files. Nothing is sent to the network.
#[test] fn owner_collections_survive_the_store_and_every_request_prepares() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spike/samples");
    let files: Vec<_> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json")).collect()).unwrap_or_default();
    if files.is_empty() { println!("owner samples: 0 (skipped)"); return; }
    let mut collections = 0;
    for (n, path) in files.iter().enumerate() {
        let Ok(src) = rqm_postman::parse_text(&std::fs::read_to_string(path).unwrap()) else { continue };
        if !matches!(rqm_postman::detect(&src), Ok(rqm_postman::Detected::Collection)) { continue; }
        collections += 1;
        let imported = rqm_postman::import_collection(&src).unwrap();
        let mut store = rqm_store::Store::open_in_memory().unwrap();
        store.insert_nodes(&imported.nodes).unwrap();
        let root = imported.nodes[0].id.clone();
        let stored = store.subtree(&root).unwrap();
        assert_eq!(stored.len(), imported.nodes.len(), "sample #{n}: node count");
        let diff = rqm_postman::semantic_diff(&src, &rqm_postman::export_collection(&stored).unwrap());
        assert!(diff.is_empty(), "sample #{n}: {} paths differ after the store", diff.len());

        let (mut prepared, mut blocked, mut with_script_warning, mut unresolved) = (0, 0, 0, 0);
        for node in stored.iter().filter(|x| x.kind == rqm_model::NodeKind::Request) {
            let input = rqm_runner::load_send_input(&store, &node.id, None, None, rqm_runner::AppSettings::default()).unwrap();
            match rqm_runner::prepare(&input, &mut |_| None) {
                Ok(p) => {
                    prepared += 1;
                    if p.warnings.iter().any(|w| matches!(w, rqm_runner::SendWarning::ScriptsNotExecuted { .. })) { with_script_warning += 1; }
                    if p.warnings.iter().any(|w| matches!(w, rqm_runner::SendWarning::UnresolvedVariables { .. })) { unresolved += 1; }
                }
                Err(_) => blocked += 1,
            }
        }
        println!("owner sample #{n}: requests prepared {prepared}, blocked {blocked}, with script warning {with_script_warning}, with unresolved variables {unresolved}, report items {}", imported.report.items.len());
        assert_eq!(prepared + blocked, stored.iter().filter(|x| x.kind == rqm_model::NodeKind::Request).count());
    }
    println!("owner collections: {collections}");
}
```

- [ ] **Step 2: `cargo test -p rqm-runner --test owner_end_to_end -- --nocapture`** — passed; для текущего образца
  в выводе `owner collections: 1` и `requests prepared` + `blocked` = 43. Числа записать в документ результатов.
- [ ] **Step 3: Убедиться, что на удаляемые пробники никто не ссылается:** поиск `postman-probe` и `http-probe`
  по репозиторию (кроме `docs/` и самих папок) ничего не находит. Удалить обе папки, обновить `spike/README.md`.
- [ ] **Step 4: Полный прогон:** `cargo test --workspace` и `cargo clippy --workspace -- -D warnings` — всё зелёное.
  `cargo build --workspace --release` — успех.
- [ ] **Step 5: Проверить, что в git нет данных владельца:** `git ls-files spike/samples` ничего не печатает,
  `git check-ignore spike/samples` срабатывает.
- [ ] **Step 6: Обновить спеку и написать документ результатов** (перечень в блоке Files).
- [ ] **Step 7: Commit** — «Этап 1A: сквозная проверка на образце владельца, удаление пробников формата и HTTP,
  итоги».
