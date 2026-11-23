# RequestMaster, этап 1, часть 3 (план 2) «Дерево, переменные, настройки, память окна» — план реализации

**Goal:** Окно закрывает оставшиеся пункты этапа 1: узлы дерева перетаскиваются, папка и коллекция редактируются
целиком, `{{переменные}}` подсвечены и подсказываются, вкладки и черновики переживают перезапуск, есть настройки
приложения, вкладки Scripts и Settings, form-data и binary, «Сохранить в файл», поиск в ответе, строгая CSP и замеры
раздела 10 спеки. Скрипты по-прежнему только редактируются и сохраняются (выполнение — этап 2).

**Architecture:** Как в плане 1: логика — в `AppCore` (`src-tauri`, Rust, без типов Tauri, под тестами), команды Tauri
только вызывают его; окно `ui/` (Svelte 5 + TypeScript) не содержит разбора Postman, подстановки переменных и HTTP.
Ядро `crates/rqm-*` меняется только там, где задача это называет (Task 1, 2, 15). Схема базы не меняется: новое
состояние — новые строки таблицы `app_state`.

**Tech Stack:** то же, что в плане 1. Новые npm-зависимости **[моё решение]**: `@codemirror/lang-javascript 6.2.5`
(подсветка скриптов; новая загрузка, тянет `@lezer/javascript`; перед закреплением сверить `npm view
@codemirror/lang-javascript version engines`); прямыми становятся уже лежащие в lock-файле `@codemirror/view 6.43.15`,
`@codemirror/autocomplete 6.20.4`, `@codemirror/search 6.7.2` (ничего не скачивается; прецедент — решение R3 этапа 1B).
Новых crate нет.

**Spec:** `docs/design.md` (разделы 4, 5, 7, 9, 10, 11, 12). План 1 этой
части и его формат: `docs/plans/stage1-part3-collections-history.md`; итоги —
`stage1-part3-results.md`.

## Global Constraints

- Ветка `feature/stage1-part3`. В `main`/`master` не коммитить. Push — никогда. Шаги «Commit» выполняются по команде владельца.
- Сообщения коммитов на русском.
- Приложение, собранный exe и dev-серверы не запускать; `npm run tauri build` — только в Task 18 и только когда
  `requestmaster.exe` не запущен. Разрешено: `cargo build/test/clippy`, в `ui/` — `npm install`, `npm run check`,
  `npm run test`, `npm run build`.
- Сеть: crates.io и npm. Тесты ходят только на `127.0.0.1`; в тестах отправки — `NO_PROXY=127.0.0.1`.
- Версии зависимостей закреплять точно; `engines` npm-пакета должен допускать Node 22.12.
- **База данных:** DDL нет, `PRAGMA user_version` остаётся `2`, `schema.sql` и `schema_v2.sql` не трогать (в том
  числе ради комментариев). Разрешены только ключи `app_state` из раздела «Изменения в базе данных». Любая таблица,
  колонка, индекс или ключ сверх этого — остановиться и спросить.
- В TypeScript нет разбора и сборки Postman-JSON, подстановки переменных, сборки HTTP-запроса, форматирования JSON.
  Поиск границ `{{…}}` в тексте для раскраски (Task 8) — разметка, не подстановка; значения и области даёт ядро.
- Тексты интерфейса — в `ui/src/lib/strings.ts` под английскими ключами, на русском; термины HTTP и Postman — на
  английском. В компонентах и в Rust русского текста нет. Цвета и размеры — только через `ui/src/styles/tokens.css`.
- В разметке Svelte нет статических атрибутов `style="…"` — только директивы `style:` (под строгой CSP атрибут
  молча не сработает; Task 16 закрепляет это тестом).
- Каждая команда окна — в четырёх местах: `COMMANDS` (`src-tauri/src/lib.rs`), `src-tauri/build.rs`,
  `generate_handler!` (`src-tauri/src/main.rs`), `capabilities/default.json` (`allow-<имя-через-дефис>`); плюс список
  и число в тесте `commands_are_listed_everywhere` (`src-tauri/tests/core_data.rs`). Сейчас команд 27; после
  Task 1 — 29, Task 2 — 34, Task 3 — 36, Task 17 — 37. Каждый новый метод `Api` появляется сразу в `tauriApi` и в
  `lib/testing/fakeApi.ts`.
- Команды — `#[tauri::command(rename_all = "snake_case")] pub async fn … -> Result<_, String>`; ключи аргументов и
  ответов — `snake_case`; замок хранилища через `.await` не удерживается. Верхний `postman_extra` узла в окно не
  передаётся. Правила P1–P4 спеки (раздел 5) соблюдаются: окно возвращает `postman_extra` строк без изменений.
- Данные владельца: `spike/samples/` не открывать, не печатать, имена файлов не упоминать; в git не попадает.
- Только то, что в задаче. Доступность: всё с клавиатуры, у кнопок-значков `aria-label`, фокус виден.
- Поддельные таймеры в тестах окна: `vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] })` включать после
  того, как экран найден через `findBy*`; дальше `await vi.advanceTimersByTimeAsync(…)` и синхронные `getBy*` без
  `waitFor`; в `afterEach` — `vi.useRealTimers()`.
- Rust-задача заканчивается зелёными `cargo test --workspace` и `cargo clippy --workspace --all-targets -- -D
  warnings`; UI-задача — зелёными `npm run check`, `npm run test`, `npm run build` в `ui/`. Существующие проверки не
  ослабляются; изменённые утверждения перечисляются в отчёте задачи с причиной.
- Мёртвый код: задача, которая что-то заменяет, удаляет старое в том же коммите после поиска по всему репозиторию
  и называет удалённое в отчёте.

## Review Focus

1. **Сохранённое состояние не должно класть запуск.** Битый JSON в `window_state`, `settings` или черновике,
   вкладка удалённого узла, черновик формы прошлой сборки (без `scripts`/`settings`), черновик — верный JSON
   неверной формы (`"abc"`, `[]`, `{ "headers": 5 }`, `{ "body": { "mode": "url_encoded" } }`) — окно открывается
   без красных уведомлений, остальные вкладки на месте, отправка работает, а негодный черновик удалён из базы и
   на следующем запуске не мешает (тесты в Task 2 и Task 10).
2. **Недопустимый бросок в дереве.** Узел в самого себя или в своего потомка, запрос в корень, бросок под
   фильтром, файл с диска над деревом во время перетаскивания узла — команда `move_node` не вызывается, импорт
   файлов работает как раньше, дерево не «мигает»; обработчик файлов на `window` не перебивает `dropEffect`
   перетаскиваемого узла (тесты в Task 1 и Task 7, шаг списка проверки в Task 18).
3. **Строка с фокусом уехала из отрисованной части дерева.** Фокус клавиатуры, поле переименования и
   перетаскиваемая строка не пропадают при прокрутке; стрелки продолжают работать; после прокрутки без фокуса в
   дерево можно попасть по Tab — в DOM всегда ровно одна строка с `tabindex="0"` (тесты в Task 6 и Task 7).
4. **Секрет не утекает через подсказку.** Значение секретной переменной и значение открытой переменной, которая
   на неё ссылается, в окно не передаются вовсе; в подсказке и в списке автодополнения — маска (тесты в Task 1 и
   Task 8).
5. **Настройки, с которыми отправка перестаёт работать.** Адрес прокси без `http://`, таймаут вне границ — отказ
   при сохранении с понятным текстом, прежние настройки остаются; нечитаемые настройки в базе — значения по
   умолчанию (тесты в Task 2 и Task 11).

## Изменения в базе данных

**DDL и DML при обновлении нет.** Таблицы, колонки и индексы не добавляются и не меняются; `PRAGMA user_version`
остаётся `2`; уже применённые версии 1 и 2 и существующие данные не трогаются; файлы `schema.sql` и `schema_v2.sql`
не правятся (в том числе устаревший комментарий таблицы `app_state` в `schema.sql`: SQLite хранит текст `CREATE
TABLE` вместе с комментариями, правка дала бы разный текст схемы у старых и новых баз — перечень ключей ведётся в
doc-комментариях констант и в спеке, раздел 5).

Используются только новые **строки** существующей таблицы `app_state` (ключ — значение JSON). Строки создаются
лениво первой записью; в старой базе их просто нет — действуют значения по умолчанию.

| Ключ | Константа (где объявлена) | Кто пишет | Кто читает | Зачем, откуда значение |
|---|---|---|---|---|
| `window_state` | `WINDOW_STATE_KEY` (`src-tauri/src/session.rs`) | команда `save_window_state` (окно, через 500 мс после изменения и при закрытии) | команда `load_session` при запуске | Раскладка окна и открытые вкладки. Без него окно открывается с раскладкой по умолчанию и без вкладок |
| `draft:<id узла>` | `DRAFT_KEY_PREFIX` (`src-tauri/src/session.rs`) | команда `save_draft` (запись — через 1 с после правки; удаление — после Ctrl+S, «Не сохранять», закрытия вкладки, а при восстановлении — черновика, который не удалось наложить на узел); `load_session` удаляет осиротевшие и те, чьё значение не JSON-объект | команда `load_session` при запуске | Несохранённый черновик открытой вкладки запроса, коллекции или папки (спека, раздел 9: «черновики хранятся в `app_state` и переживают перезапуск»). По ключу на узел — чтобы правка одной вкладки не переписывала чужие большие тела |
| `settings` | `SETTINGS_KEY` (`src-tauri/src/settings.rs`) | команда `save_settings` (окно настроек) | `get_settings`; `send_request` перед каждой отправкой | Общие настройки отправки: таймаут, проверка сертификата, прокси (спека, раздел 7) |

Форма значений:

```json
// window_state
{ "sidebar_width": 280, "placement": "auto", "split": 50,
  "tabs": [ { "kind": "node", "id": "<id узла>" }, { "kind": "environment", "id": "globals" } ],
  "active": "env:globals" }
// draft:<id узла> — содержимое узла в том виде, как его держит редактор окна (без верхнего postman_extra)
{ "method": "POST", "url": "{{host}}/login", "headers": [ … ], "body": { "mode": "raw", "data": { … } }, … }
// settings
{ "timeout_ms": 0, "verify_tls": true, "proxy": { "mode": "system" } }
//   proxy: { "mode": "system" } | { "mode": "off" } | { "mode": "manual", "url": "http://host:port" }
```

Единственный DML по ходу работы программы — `INSERT … ON CONFLICT DO UPDATE` (уже есть, `state_set`) и новый
`DELETE FROM app_state WHERE key = ?1` для черновиков (`state_delete`). Существующие ключи `cookie_jar` и
`active_environment` не меняются. Секреты: черновик с токеном лежит в базе открытым текстом — как и остальные данные
(спека, раздел 11). В режиме замера (Task 17, переменная окружения `RQM_MEASURE_DIR`) программа открывает **другой
файл базы** в указанной папке; рабочая база владельца при этом не открывается и не меняется.

## Контракт команд

Напоминание: каждая новая команда — в четырёх местах (`COMMANDS`, `build.rs`, `generate_handler!`,
`capabilities/default.json`) плюс список и число в `commands_are_listed_everywhere`, плюс `Api`, `tauriApi`, `fakeApi`.

| № | Команда (Rust, `commands.rs`) | Метод `AppCore` | Метод `Api` (TS) и `invoke` | Задача |
|---|---|---|---|---|
| 1 | `move_node(core, id: String, parent_id: Option<String>, index: usize) -> Result<(), String>` | `move_node(&self, id: &str, parent_id: Option<&str>, index: usize) -> Result<(), String>` | `moveNode(id: string, parentId: string \| null, index: number): Promise<void>` — `{ id, parent_id, index }` | 1, 7 |
| 2 | `variables_in_scope(core, node_id: String) -> Result<Vec<ScopeVariable>, String>` | `variables_in_scope(&self, node_id: &str) -> Result<Vec<ScopeVariable>, String>` | `variablesInScope(nodeId: string): Promise<ScopeVariable[]>` — `{ node_id }` | 1, 8 |
| 3 | `load_session(core) -> Result<SessionDto, String>` | `load_session(&self) -> Result<SessionDto, String>` | `loadSession(): Promise<SessionDto>` | 2, 10 |
| 4 | `save_window_state(core, state: WindowState) -> Result<(), String>` | `save_window_state(&self, state: WindowState) -> Result<(), String>` | `saveWindowState(state: WindowState): Promise<void>` — `{ state }` | 2, 10 |
| 5 | `save_draft(core, node_id: String, content: Option<serde_json::Value>) -> Result<(), String>` | `save_draft(&self, node_id: &str, content: Option<serde_json::Value>) -> Result<(), String>` | `saveDraft(nodeId: string, content: NodeContent \| null): Promise<void>` — `{ node_id, content }` | 2, 10 |
| 6 | `get_settings(core) -> Result<SettingsDto, String>` | `get_settings(&self) -> Result<SettingsDto, String>` | `getSettings(): Promise<SettingsDto>` | 2, 11 |
| 7 | `save_settings(core, settings: SettingsDto) -> Result<(), String>` | `save_settings(&self, settings: SettingsDto) -> Result<(), String>` | `saveSettings(settings: SettingsDto): Promise<void>` — `{ settings }` | 2, 11 |
| 8 | `save_response_body(window: tauri::WebviewWindow, core, body_id: String) -> Result<Option<String>, String>` | `response_file_name(&self, body_id: &str) -> Result<String, String>`; `save_response_body(&self, body_id: &str, target: &Path) -> Result<(), String>` | `saveResponseBody(bodyId: string): Promise<string \| null>` — `{ body_id }`; `null` — окно выбора закрыто | 3, 13 |
| 9 | `pick_files(window: tauri::WebviewWindow, multiple: bool) -> Result<Vec<String>, String>` | — (только системное окно) | `pickFiles(multiple: boolean): Promise<string[]>` — `{ multiple }`; пустой список — отмена | 3, 12 |
| 10 | `report_timings(core, samples: Vec<TimingSample>) -> Result<(), String>` | `report_timings(&self, samples: Vec<TimingSample>) -> Result<(), String>` | `reportTimings(samples: TimingSample[]): Promise<void>` — `{ samples }` | 17 |

JSON ответов и аргументов (ключи всегда присутствуют, если не сказано иное):

```text
ScopeVariable = { name: string, scope: "global"|"collection"|"environment"|"dynamic",
                  value: string|null,   // итоговое значение после полной подстановки; null — секрет или динамическая
                  secret: boolean }     // true — значение (или то, на что оно ссылается) секретно
SessionDto    = { window: WindowState, drafts: DraftDto[] }          // Task 17 добавляет measure: boolean
WindowState   = { sidebar_width: number, placement: string, split: number, tabs: SavedTab[], active: string|null }
SavedTab      = { kind: "node"|"environment", id: string }           // node — запрос, коллекция или папка
DraftDto      = { node_id: string, content: <содержимое узла как в get_node> }
SettingsDto   = { timeout_ms: number, verify_tls: boolean,
                  proxy: { mode: "system" } | { mode: "off" } | { mode: "manual", url: string } }
TimingSample  = { name: string, ms: number }
```

Изменённые команды: `export_collection` и `export_environment` — параметр `app: tauri::AppHandle` заменён на `window:
tauri::WebviewWindow` (окно «Сохранить как» привязано к главному окну; аргументы и ответ для окна прежние, Task 3).
`get_node` и `save_node_content` — у коллекции и папки в `content` появляется ключ `description` (строка; Task 15).
Новых разрешений ядра Tauri и плагинов в capability нет: системные окна вызывает Rust-сторона.

## Решения автора

Каждое владелец может отклонить; после знака «/» — альтернатива.

| № | Решение | Почему / альтернатива |
|---|---|---|
| 1 | Описание коллекции и папки — отдельная Task 15, её можно выбросить целиком. Хранится там же, где у Postman (в исходном объекте узла), отдельного поля модели нет | В спеке (раздел 9) описания в редакторе нет, а в постановке есть. Так экспорт не меняется и ничего не дублируется / типизированное поле в `rqm-model` с правкой импорта и экспорта; либо не делать |
| 2 | У папки переменных нет: редактор папки — Auth, Scripts (и описание) | В модели, спеке и `rqm-vars` области «папка» нет / новая область — изменение ядра и экспорта |
| 3 | Папка открывается пунктом меню строки «Открыть»; щелчок по папке по-прежнему только сворачивает | Щелчок-открытие заваливал бы вкладками при обычном просмотре дерева / открывать щелчком, как коллекцию |
| 4 | Перемещение с клавиатуры: Alt+↑/↓ — среди соседей, Alt+→ — внутрь предыдущей соседней папки, Alt+← — наружу, сразу после родителя | Правило проекта «всё с клавиатуры», а перетаскивание мышью с клавиатуры недоступно / пункт меню «Переместить…» с окном выбора; не делать |
| 5 | Под фильтром перетаскивание и Alt+стрелки выключены | Видна часть детей — место вставки неоднозначно / определить правило вставки |
| 6 | Линия вставки появляется без анимации, строки переставляются без анимации | «Анимации сверх простых» исключены из плана / проявление `opacity` |
| 7 | Нет авто-разворачивания папки при наведении и своей автопрокрутки при перетаскивании | Бросок «внутрь» свёрнутой папки работает; автопрокрутку у края даёт сам WebView2 (шаг списка проверки) / добавить |
| 8 | Виртуализация дерева — своя, без библиотеки; когда высота области неизвестна (среда тестов) — рисуются все строки | Строка фиксирована, 30 px; 60 существующих проверок не переписываются / библиотека |
| 9 | Перемещение применяется в окне сразу, запись следом; при отказе — уведомление и перечитывание дерева | Спека, раздел 9 / ждать команду, как при создании и удалении |
| 10 | Подсветка в однострочных полях — слой-двойник под прозрачным текстом поля | Редактор кода не грузится при открытии запроса, сотни строк таблицы не создают сотни редакторов / CodeMirror в каждом поле |
| 11 | Счётчик у Send считает окно: имена `{{…}}` из включённых мест этого запроса, которых нет среди переменных области. Переменные унаследованного auth и вложенные `{{a{{b}}}}` он не видит — предупреждение после отправки может назвать больше | Без вызова ядра на каждое нажатие и без пересылки тела / команда «сухого прогона» |
| 12 | Подсветка есть в URL, Params, Headers, Body (raw, urlencoded, текст form-data). Нет в Auth, скриптах, редакторе окружений; `/:id` не подсвечивается; вспышки при смене окружения нет | Спека, раздел 9 называет URL, заголовки, тело; постановка добавляет Params / расширить |
| 13 | Значение секретной переменной в окно не передаётся; в подсказке — маска. Наведение — только мышью; с клавиатуры то же показывает список по Ctrl+Space | Спека, раздел 5: секрет в интерфейсе замаскирован / показывать по кнопке «Показать» |
| 14 | Цвета областей — новые токены `--var-global #e5a93b`, `--var-collection #a98bf5`, `--var-environment #4cc38a`, `--var-dynamic #6aa6f8`; ненайденная — `--danger` и подчёркивание | Цвета палитры «Графит», контраст проверяется тестом / другие значения |
| 15 | Состояние окна — один ключ `window_state`, черновики — по ключу на узел | См. таблицу базы / всё одним значением |
| 16 | Вкладки истории после перезапуска не восстанавливаются; вкладки окружений — да | Запись истории могла быть вытеснена или очищена / восстанавливать |
| 17 | Вопрос «Закрыть приложение?» о несохранённых вкладках убран: черновики переживают перезапуск. Вопрос остаётся только когда что-то не удалось записать — черновик, состояние окна **или окружение** («Закрыть без сохранения» / «Отмена»). Для окружения это меняет нынешнее поведение: сейчас окно при таком сбое просто не закрывается. Вопрос показывается при первом же сбое (в итогах плана 1, вопрос 4, предлагалось «при повторном» и стояло «подтвердить» — в пункты A–I постановки это не входит, поэтому решение вынесено сюда). Так закрывается отложенное T-n для закрытия окна; T-n для закрытия **вкладки** окружения остаётся отложенным | Спека, раздел 9; иначе после снятия прежнего вопроса сбой сохранения окружения оставался бы единственным случаем, когда окно молча не закрывается / не трогать: отказ сохранения окружения по-прежнему оставляет окно открытым без вопроса (тогда T-n убирается из № 37, Task 10 и Task 18 Step 5, а существующий тест `Environments.test.ts` остаётся как есть); либо прежний вопрос о вкладках |
| 18 | Значение `rqm.layout` из `localStorage` не переносится: расположение ответа нужно выбрать один раз заново | Перенос оставил бы код чтения `localStorage` навсегда / разовый перенос при первом запуске |
| 19 | Запись состояния окна — через 500 мс после изменения, черновика — через 1 с, обе — при закрытии окна | Перетаскивание разделителя больше не пишет на каждое движение мыши / другие задержки |
| 20 | Не запоминаются: раскрытые узлы дерева, раздел боковой панели, размер и положение окна Windows, ответ | В спеке и постановке не названы; файл тела ответа удаляется при запуске / добавить ключи |
| 21 | Общих настроек три: таймаут, проверка сертификата, прокси. «Следовать редиректам» и «максимум редиректов» остаются настройками запроса | Спека, раздел 7 / добавить общие — изменение `rqm-runner` и спеки |
| 22 | Таймаут — в миллисекундах, `0` — без ограничения, максимум 3 600 000 (час) | Как в Postman; граница от опечаток / секунды, другая граница |
| 23 | Адрес прокси проверяется при сохранении (`http://` или `https://` с хостом) | Иначе ошибка всплывает при отправке как «неверный адрес» запроса / проверять при отправке |
| 24 | Нечитаемое значение `active_environment` теперь значит «окружение не выбрано», а не ошибку (отложенное W-b) | Иначе любая отправка невозможна / оставить |
| 25 | Подсветка JavaScript в Scripts — новая зависимость; Tab в редакторе скрипта уводит фокус (как сейчас в теле) | Спека, раздел 3 / без подсветки до этапа 2; Tab как отступ |
| 26 | Счётчики вкладок: Scripts — число непустых скриптов (0–2), Settings — число заданных настроек | Спека: «со счётчиками» / точка вместо числа |
| 27 | Настройки запроса — три состояния («по умолчанию / да / нет»); максимум редиректов — целое 0–9999 | Импортированное `false` не должно превращаться в «не задано» (P2) / галочки |
| 28 | «Сохранить в файл» пишет тело как получено и распаковано (не Pretty); имя по умолчанию `response.<расширение по Content-Type>` | Работает и для тел больше 20 МиБ / сохранять показанный вид |
| 29 | Файл для form-data и binary выбирается только системным окном; перетаскивания файла на поле и ручного ввода пути нет | WebView пути к файлу не даёт; `confirmed: true` — только для выбранного в окне / — |
| 30 | GraphQL остаётся только для чтения (с прежним вопросом «Заменить тело?») | В постановке не назван / редактор GraphQL |
| 31 | Ctrl+F: фокус в любом редакторе кода — поиск в нём; иначе — поиск в теле ответа показанной вкладки; отдельной кнопки поиска нет | Спека, раздел 9 / кнопка в панели ответа |
| 32 | Окружение с пустым именем: имя «Без названия» подставляет окно; в базу оно попадает при первой правке | Чинит и уже импортированные окружения без правки данных; в Rust нет русских строк / подставлять при импорте |
| 33 | Рамка фокуса на вкладке: окно помнит, что фокус пришёл от мыши, и гасит рамку; клавиатурная остаётся | `:focus-visible` уже стоит глобально, причина — эвристика браузера / снимать фокус после щелчка |
| 34 | CSP: nonce Tauri для стилей CodeMirror; добавлены `form-action 'none'; frame-src 'none'; worker-src 'none'` | Работает при обоих прочтениях поведения Tauri; откат — две строки `tauri.conf.json` / Shadow DOM |
| 35 | Режим замера: переменная окружения `RQM_MEASURE_DIR=<папка>` — отдельная база и папка тел в ней, файл `timings.tsv`; команда `report_timings`. Без переменной программа ведёт себя как обычно. Скрипт `scripts/measure-start.ps1` умеет три вещи: серия холодных запусков, запуск для ручной работы (`-Keep`) и сводка по файлу замеров (`-Report`) | Замеры раздела 10 требуют сигнала из окна и не должны трогать рабочую базу / без кода продукта — только «появилось окно» |
| 36 | Замеры «накладные расходы» и «3000 запросов» — тесты с `#[ignore]`, запускаются вручную в release-сборке (`cargo test --release`) и печатают число | Порог по времени в обычном прогоне был бы хрупким; отладочная сборка в разы медленнее и с целью спеки несравнима / обычные тесты со щедрой границей |
| 37 | Попутно закрываются отложенные замечания: W-b, W-l, W-o, T-n (только для закрытия окна, см. № 17), T-s, T-t | Затрагивают те же файлы / оставить финальному ревью |
| 38 | Цель «отклик на ввод ≤ 16 мс» меряется так: медиана времени обработки события (`processingEnd − startTime`, строки `…:proc`) ≤ 16 мс и ни одной записи `duration` больше 32 мс (отклик попал в ближайший кадр). Это уточнение записывается в раздел 10 спеки | `duration` Event Timing — время до следующей отрисовки с шагом 8 мс: на мониторе 60 Гц даже обработчик в 1 мс даёт 16–24 мс, цель «по `duration` ≤ 16» проваливалась бы без вины кода / сравнивать `duration` с 16 мс; мерить `performance.now()` до и после двух кадров |
| 39 | Черновик, который не удаётся наложить на содержимое узла (верный JSON неверной формы), молча отбрасывается и удаляется из базы; вкладка открывается с сохранённым содержимым. Полной проверки формы черновика нет: значение, которое нормализуется без ошибки, принимается как есть | Запуск не должен падать из-за одного ключа (Review Focus № 1); такой черновик могла оставить только другая сборка или правка базы руками / показывать уведомление «черновик не восстановлен»; проверять форму черновика целиком в ядре |

Решение владельца (не автора), зафиксировано без изменения кода: предел тела в истории остаётся 1 МиБ, худший размер
истории — около 1 ГБ (пункт I(2); вопрос 2 раздела 8 итогов плана 1 закрыт).

## Не входит в план

- Исключено постановкой: Preview HTML, сворачивание боковой панели, анимации сверх простых, события прогресса
  отправки, выполнение скриптов и настройка «разрешить скрипты» у коллекции, командная работа.
- Требования этапа 1 из спеки, которых нет в постановке (остаются нераспределёнными): автозаголовки отдельной
  группой во вкладке Headers; просмотр и очистка общего cookie jar; Ctrl+T; виртуализация таблиц; редактор GraphQL;
  подсветка XML/HTML в raw; общие «следовать редиректам» и «максимум редиректов».
- Отложено решениями этого плана: переменные папки; авто-разворачивание и автопрокрутка при перетаскивании;
  восстановление вкладок истории, раскрытых узлов, размера окна; перенос `rqm.layout`; подсветка переменных в Auth и
  в редакторе окружений.
- Предлагаю от себя (не делается): заметная пометка в ответе, когда проверка сертификата выключена; имя файла
  экспорта для окружения с пустым именем; та же подстановка имени для коллекции и узлов с пустым именем.
- Остаются отложенными: T-a, T-b, T-c, T-d, T-e–T-k, T-m, T-n в части закрытия **вкладки** окружения (окружение,
  которое не сохраняется, по-прежнему не даёт закрыть свою вкладку), T-o, T-p, T-q, T-r, T-u–T-y, W-a, W-c, W-d,
  W-f–W-j, W-m, W-n, W-p, P-*.

---

### Task 1: Оболочка — перемещение узла и переменные области

**Files:**
- Modify: `crates/rqm-vars/src/dynamic.rs`, `crates/rqm-vars/src/lib.rs`, `crates/rqm-vars/tests/substitute.rs`
- Create: `crates/rqm-runner/src/scope.rs`, `crates/rqm-runner/tests/scope.rs`
- Modify: `crates/rqm-runner/src/lib.rs`, `crates/rqm-runner/src/prepare.rs`, `crates/rqm-store/tests/store.rs`
- Create: `src-tauri/src/variables.rs`
- Modify: `src-tauri/src/lib.rs`, `core.rs`, `dto.rs`, `commands.rs`, `main.rs`, `build.rs`,
  `capabilities/default.json`
- Test: `src-tauri/tests/core_tree.rs`, `src-tauri/tests/core_data.rs`

**Interfaces:**
- Consumes: `Store::move_node(id, new_parent, index)` (уже есть; `index` — позиция в списке детей нового родителя
  **без** перемещаемого узла), `Store::ancestors`, `Store::node`, `Store::environments`, `Store::globals`,
  `active_environment_in(store)`.
- Produces:

```rust
// crates/rqm-vars
/// Names `system_dynamic` knows, without the `$`.
pub const DYNAMIC_NAMES: [&str; 5] = ["guid", "randomUUID", "timestamp", "isoTimestamp", "randomInt"];
impl Resolver { /// Every effective variable as (name, raw value, scope), sorted by name.
                pub fn entries(&self) -> Vec<(&str, &str, ScopeKind)>; }

// crates/rqm-runner/src/scope.rs, re-exported from lib.rs
/// The variables of the three stored scopes around a node, disabled ones included.
pub struct ScopeSources { pub globals: Vec<Variable>, pub collection: Vec<Variable>, pub environment: Vec<Variable> }
impl ScopeSources {
    pub fn of_input(input: &SendInput) -> Self;
    /// `node_id` is a request, a folder or a collection: the collection is its first ancestor or the node itself.
    /// An unknown node or environment is `StoreError::NotFound`.
    pub fn load(store: &Store, node_id: &str, environment_id: Option<&str>) -> Result<Self, StoreError>;
    /// Enabled variables only, the widest scope first — the resolver of a send.
    pub fn resolver(&self) -> Resolver;
}
pub enum VariableOrigin { Global, Collection, Environment, Dynamic }
pub struct ScopeVariable { pub name: String, pub origin: VariableOrigin, pub value: Option<String>, pub secret: bool }
pub fn variables_in_scope(sources: &ScopeSources) -> Vec<ScopeVariable>;

// src-tauri
impl AppCore {
    pub fn move_node(&self, id: &str, parent_id: Option<&str>, index: usize) -> Result<(), String>;
    pub fn variables_in_scope(&self, node_id: &str) -> Result<Vec<dto::ScopeVariable>, String>;   // variables.rs
}
// dto.rs: ScopeVariable { name, scope: VariableScope, value: Option<String>, secret: bool },
//         #[serde(rename_all = "snake_case")] enum VariableScope { Global, Collection, Environment, Dynamic }
```

**Правила:**
- `prepare.rs`: приватная `resolver(input)` удаляется, `prepare` берёт `ScopeSources::of_input(input).resolver()` —
  отправка и подсветка собирают области одним кодом.
- `variables_in_scope`: по одной записи на имя из `resolver().entries()`; `value` — результат
  `substitute_with("{{имя}}", dynamic = всегда None)`. Секретность: вторая подстановка тем же способом, но значения
  всех секретных переменных заменены меткой `"\u{0}secret\u{0}"`; результаты различаются → `secret = true`,
  `value = None`. Затем имена `"$" + DYNAMIC_NAMES[i]`, не перекрытые переменной пользователя: `origin = Dynamic`,
  `value = None`, `secret = false`. Порядок: переменные по имени, затем динамические в порядке `DYNAMIC_NAMES`.
- `AppCore::move_node`: `StoreError::InvalidMove(rule) => rule` (без префикса `invalid move:`, как в `create_node`),
  остальное — `message`. `AppCore::variables_in_scope` берёт активное окружение `active_environment_in`.

- [ ] **Step 1: Падающие тесты.**
  `rqm-vars/tests/substitute.rs`: `dynamic_names_all_resolve` (каждое имя `DYNAMIC_NAMES` → `system_dynamic(name)
  .is_some()`, `system_dynamic("nope").is_none()`); `entries_are_sorted_by_name`.
  `rqm-runner/tests/scope.rs`: `narrowest_scope_wins_and_value_is_fully_substituted` (globals `host=g`; collection
  `host=c`, `url={{host}}/api`; environment `host=e` → `host`: `Environment`, `Some("e")`; `url`: `Collection`,
  `Some("e/api")`); `disabled_variables_are_left_out`; `secret_is_never_returned_even_through_a_reference`
  (environment `token` секретная `s3cr3t`; collection `auth=Bearer {{token}}` не секретная → у обеих `secret ==
  true`, `value == None`; `format!("{:?}", all)` не содержит `s3cr3t`); `dynamic_names_are_listed_unless_shadowed`
  (пять записей `$guid`…`$randomInt` с `Dynamic`; при переменной `$guid` — она `Global`, динамических четыре);
  `request_folder_and_collection_share_one_collection_scope`; `cycle_terminates_and_keeps_the_token` (`a={{b}}`,
  `b={{a}}` → `value` содержит `{{`); `scope_and_send_agree` (URL `{{a}}/{{b}}`, задана только `a` → предупреждение
  `UnresolvedVariables { names: ["b"] }` и в `variables_in_scope` есть `a`, нет `b`).
  `rqm-store/tests/store.rs`: `move_within_a_parent_and_between_collections` (дети `C`: `[F1, R2, R3]`;
  `move_node(F1, Some(C), 1)` → `[R2, F1, R3]`; `move_node(R3, Some(C), 0)` → `[R3, R2, F1]`; две коллекции:
  `move_node(c2, None, 0)` → `c2` первая; запрос в другую коллекцию; `sort_order` везде `0..n`).
  `core_tree.rs`: `move_node_reorders_and_moves`; `move_node_refuses_forbidden_places_and_changes_nothing` (в себя, в
  потомка, запрос в корень, коллекция под папку, родитель-запрос: текст ошибки — само правило, например `a node
  cannot be moved into itself or its descendant`; дерево прежнее); `moved_request_sees_the_variables_of_its_new_
  collection`; `variables_in_scope_uses_the_active_environment` (JSON записи: `{"name":"host","scope":"environment",
  "value":"e","secret":false}`); `variables_in_scope_of_an_unknown_node_is_an_error`.
  `core_data.rs`: `commands_are_listed_everywhere` — добавить `move_node`, `variables_in_scope`, число 29.
- [ ] **Step 2: Запустить** `cargo test -p rqm-vars -p rqm-runner -p rqm-store -p requestmaster` — новые тесты не
  компилируются или падают.
- [ ] **Step 3: Реализовать** по сигнатурам блока Interfaces; две команды в четырёх местах.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные; числа в отчёт.
- [ ] **Step 5: Commit** — «План 2: команды перемещения узла и переменных области».

---

### Task 2: Оболочка — состояние окна, черновики, настройки приложения

**Files:**
- Modify: `crates/rqm-store/src/state.rs`, `crates/rqm-store/tests/store.rs`
- Create: `src-tauri/src/session.rs`, `src-tauri/src/settings.rs`, `src-tauri/tests/core_session.rs`
- Modify: `src-tauri/src/lib.rs`, `core.rs`, `send.rs`, `dto.rs`, `commands.rs`, `main.rs`, `build.rs`,
  `capabilities/default.json`
- Test: `src-tauri/tests/core_send.rs`, `src-tauri/tests/core_data.rs`

**Interfaces:**
- Consumes: `Store::state_get/state_set`, `rqm_runner::AppSettings { timeout: Option<Duration>, verify_tls: bool,
  proxy: ProxySetting }`, `rqm_http::resolve_proxy(&ProxySetting, Option<String>) -> Result<Option<String>, HttpError>`.
- Produces:

```rust
// crates/rqm-store/src/state.rs
/// Removes the key; a key that was never written is not an error.
pub fn state_delete(&mut self, key: &str) -> Result<(), StoreError>;
/// `(key, value)` of every key that starts with `prefix` (compared literally), ordered by key.
pub fn state_entries(&self, prefix: &str) -> Result<Vec<(String, String)>, StoreError>;

// src-tauri/src/session.rs
pub const WINDOW_STATE_KEY: &str = "window_state";
pub const DRAFT_KEY_PREFIX: &str = "draft:";
impl AppCore { pub fn load_session(&self) -> Result<SessionDto, String>;
               pub fn save_window_state(&self, state: WindowState) -> Result<(), String>;
               pub fn save_draft(&self, node_id: &str, content: Option<serde_json::Value>) -> Result<(), String>; }
// src-tauri/src/settings.rs
pub const SETTINGS_KEY: &str = "settings";
pub const MAX_TIMEOUT_MS: u64 = 3_600_000;
impl AppCore { pub fn get_settings(&self) -> Result<SettingsDto, String>;
               pub fn save_settings(&self, settings: SettingsDto) -> Result<(), String>; }
/// The stored settings as the send takes them; absent or unreadable — `AppSettings::default()`.
pub(crate) fn app_settings_in(store: &Store) -> AppSettings;
// dto.rs — формы из раздела «Контракт команд»:
// SessionDto { window: WindowState, drafts: Vec<DraftDto> }
// WindowState { sidebar_width: f64, placement: String, split: f64, tabs: Vec<SavedTab>, active: Option<String> }
// SavedTab { kind: SavedTabKind /* Node | Environment, snake_case */, id: String }
// DraftDto { node_id: String, content: serde_json::Value }
// SettingsDto { timeout_ms: u64, verify_tls: bool, proxy: ProxyDto }
// #[serde(tag = "mode", rename_all = "snake_case")] enum ProxyDto { System, Off, Manual { url: String } }
```

**Правила:**
- `load_session`: нет ключа или значение не читается → `WindowState { 280, "auto", 50, [], None }`. Из `tabs`
  выбрасываются вкладки узлов, которых нет в дереве, и окружений, которых нет (id `globals` есть всегда); `active`,
  указывающий на выброшенную вкладку (`<id узла>` или `env:<id>`), → `None`. Черновики: для каждого ключа
  `draft:<id>` — узел есть и значение разбирается как JSON-объект (`serde_json::Value::Object`) → попадает в
  `drafts`; иначе (узла нет, не JSON, либо JSON, но не объект: строка, число, массив, `null`) ключ удаляется.
  Ошибку возвращает только отказ самого хранилища.
- `save_window_state`: значение пишется как пришло. `save_draft`: `Some` — запись, если узел существует (нет узла —
  ничего не пишется, `Ok`); `None` — удаление ключа. Поля черновика ядро не проверяет (объект с полями не того
  вида отбрасывает окно при восстановлении — Task 10).
- `save_settings`: `timeout_ms > MAX_TIMEOUT_MS` → `Err("timeout is out of range")`; `Manual { url }` — `url`
  обрезается по краям и проверяется `resolve_proxy(&ProxySetting::Manual(url), None)`, отказ →
  `Err(format!("invalid proxy address: {reason}"))`; при отказе сохранённое не меняется. `get_settings`: нет ключа
  или не читается → `{ timeout_ms: 0, verify_tls: true, proxy: System }`. `timeout_ms == 0` ↔ `timeout: None`.
- `send_request` (`send.rs`): вместо `AppSettings::default()` — `app_settings_in(&store)` в том же блоке, где замок
  уже взят. Настройка действует со следующей отправки, без перезапуска.
- `active_environment_in` (`core.rs`): значение, которое не читается как JSON, → `Ok(None)` (отложенное W-b).

- [ ] **Step 1: Падающие тесты.**
  `store.rs`: `state_delete_and_entries` (удаление несуществующего ключа — `Ok`; `state_entries("draft:")` отдаёт
  только `draft:a`, `draft:b` по порядку и не отдаёт `drafts`; префикс `a%` не работает как шаблон).
  `core_session.rs` (база в файле; «битые» значения пишутся вторым соединением `rusqlite`, как в `core_history.rs`):
  `empty_database_gives_the_default_session`; `window_state_round_trips` (JSON из раздела базы — слово в слово);
  `broken_window_state_gives_defaults`; `tabs_of_deleted_nodes_and_environments_are_left_out_and_active_is_cleared`;
  `draft_round_trips_and_null_deletes_it`; `draft_of_a_deleted_node_is_dropped_and_its_key_is_gone`;
  `draft_that_is_not_json_is_dropped`; `draft_that_is_not_an_object_is_dropped` (значения `"abc"`, `[]`, `5`,
  `null` под четырьмя ключами `draft:<id>` существующих узлов и верный черновик-объект пятого узла → в `drafts`
  только пятый, четыре ключа удалены, пятый на месте); `save_draft_for_an_unknown_node_writes_nothing`;
  `session_survives_reopening_the_database`; `default_settings`; `settings_round_trip`
  (`{"timeout_ms":30000,"verify_tls":false,"proxy":{"mode":"manual","url":"http://127.0.0.1:8888"}}`);
  `settings_refuse_a_bad_proxy_and_a_huge_timeout` (`host:8080`, `socks5://h:1`, `3600001` → `Err`, сохранённое
  прежнее); `settings_dto_maps_to_app_settings` (`0` → `None`, `1500` → `Some(1500 ms)`, `off` → `ProxySetting::Off`).
  `core_send.rs`: `saved_timeout_stops_a_slow_server` (`timeout_ms: 200`, ответ wiremock с задержкой 2 с →
  `error.kind == "timeout"`); `saved_manual_proxy_is_used` (прокси — адрес wiremock, запрос на
  `http://example.invalid/x` → ответ wiremock); `broken_settings_do_not_stop_a_send`;
  `unreadable_active_environment_counts_as_none`.
  `core_data.rs`: пять новых имён, число 34.
- [ ] **Step 2: Запустить** `cargo test -p rqm-store -p requestmaster` — новые тесты падают.
- [ ] **Step 3: Реализовать**; пять команд в четырёх местах.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «План 2: состояние окна, черновики и настройки приложения в базе».

---

### Task 3: Оболочка — сохранение тела ответа, выбор файлов, окна поверх главного

**Files:**
- Modify: `src-tauri/src/send.rs`, `commands.rs`, `lib.rs`, `main.rs`, `build.rs`, `capabilities/default.json`
- Test: `src-tauri/tests/core_send.rs`, `src-tauri/tests/core_data.rs`

**Interfaces:**
- Consumes: `SendState.bodies`, `tauri_plugin_dialog::DialogExt` (`set_parent`, `blocking_save_file`,
  `blocking_pick_file`, `blocking_pick_files`, `FilePath::into_path`).
- Produces:

```rust
// send.rs — SendState.bodies: Mutex<HashMap<String, StoredBody>>, struct StoredBody { path: PathBuf, extension: &'static str }
impl AppCore {
    /// `response.<extension>`; an unknown id is the error "response body is gone".
    pub fn response_file_name(&self, body_id: &str) -> Result<String, String>;
    /// Copies the body as received and decoded to `target` with `std::fs::copy`, never through memory.
    pub fn save_response_body(&self, body_id: &str, target: &Path) -> Result<(), String>;
}
pub(crate) fn body_extension(content_type: Option<&str>, json: bool) -> &'static str;
// commands.rs
fn ask_save_path(window: &tauri::WebviewWindow, file_name: &str, extension: &str) -> Result<Option<PathBuf>, String>;
pub async fn save_response_body(window: tauri::WebviewWindow, core: State<'_, AppCore>, body_id: String) -> Result<Option<String>, String>;
pub async fn pick_files(window: tauri::WebviewWindow, multiple: bool) -> Result<Vec<String>, String>;
```

**Правила:**
- `body_extension`: `json == true` или тип содержит `json` → `"json"`; `text/html` → `"html"`; содержит `xml` →
  `"xml"`; начинается с `text/` → `"txt"`; иначе `"bin"`. Расширение запоминается в `register_body` по заголовку
  `Content-Type` ответа.
- `save_response_body`: цель совпадает с самим файлом тела (после `canonicalize`) → `Err("cannot save the response
  onto itself")`, файл тела цел; ошибки копирования — `cannot write the file: …`.
- `ask_save_path` заменяет `save_as`: `window.dialog().file().set_parent(window).set_file_name(…).add_filter(…)
  .blocking_save_file()`; закрыли окно → `Ok(None)`. `export_collection`, `export_environment` переходят на него
  (`window: tauri::WebviewWindow` вместо `app`, запись — `std::fs::write`); `save_as` удаляется. `pick_files` —
  `set_parent(window)`, один файл или несколько, пути через `to_string_lossy`; отмена — пустой список.
- Системные окна автотестом не покрываются (нужен живой `WebviewWindow`) — шаги списка проверки (Task 18).

- [ ] **Step 1: Падающие тесты** `core_send.rs`: `response_body_is_saved_as_received` (JSON-ответ: сохранённые байты
  равны байтам сервера, а не Pretty); `body_above_the_inline_limit_is_saved_whole` (ответ 21 МиБ: `body.truncated ==
  true`, длина файла `22_020_096`); `saving_a_released_body_is_an_error` (`"response body is gone"`);
  `saving_onto_the_body_file_itself_is_refused_and_keeps_it`; `response_file_name_follows_the_content_type`;
  модульный `body_extension_table`: `(Some("application/json; charset=utf-8"), false)` → `json`, `(None, true)` →
  `json`, `(Some("application/problem+json"), false)` → `json`, `(Some("text/html; charset=utf-8"), false)` → `html`,
  `(Some("text/xml"), false)` → `xml`, `(Some("text/plain"), false)` → `txt`, `(Some("image/png"), false)` → `bin`,
  `(None, false)` → `bin`. `core_data.rs`: `save_response_body`, `pick_files`, число 36.
- [ ] **Step 2: Запустить** `cargo test -p requestmaster --test core_send` — падают.
- [ ] **Step 3: Реализовать**; поиск `save_as` по репозиторию — совпадений не осталось.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «План 2: сохранение тела ответа в файл, выбор файлов, окна поверх главного».

---

### Task 4: Вкладки Scripts и Settings запроса

**Files:**
- Modify: `ui/package.json` (+ lock), `ui/src/lib/types.ts`, `ui/src/lib/strings.ts`,
  `ui/src/components/CodeEditor.svelte`, `ui/src/lib/testing/CodeEditorStub.svelte`,
  `ui/src/components/RequestEditor.svelte`, `ui/src/components/BodyEditor.svelte`
- Create: `ui/src/components/ScriptsEditor.svelte`, `ui/src/components/RequestSettingsEditor.svelte`
- Test: `ui/src/components/RequestEditor.test.ts`, `ui/src/lib/tabs.test.ts`, `src-tauri/tests/core_data.rs`

**Interfaces:**
- Consumes: `get_node` / `save_node_content` / `send_request(draft)` — без изменений; ядро принимает `null` как «не
  задано» и пустые строки скриптов.
- Produces:

```ts
// lib/types.ts
export type ScriptsDraft = Required<Scripts>;                 // { pre_request: string; test: string }
export interface RequestSettingsDraft { follow_redirects: boolean | null; max_redirects: number | null;
                                        strict_ssl: boolean | null; disable_body_pruning: boolean | null }
// RequestSettings: каждое поле `?: T | null`.
// RequestDraft: + scripts: ScriptsDraft; settings: RequestSettingsDraft.  CollectionDraft: + scripts: ScriptsDraft.
// normalizeContent("request" | "collection", …) дописывает их: пустые строки и null.
// components
ScriptsEditor:         { scripts: ScriptsDraft }               // $bindable
RequestSettingsEditor: { settings: RequestSettingsDraft }      // $bindable
CodeEditor / CodeEditorStub: language?: "json" | "javascript" | "text"
```

**Поведение:**
- `RequestEditor`: разделы `params / headers / body / auth / scripts / settings`; подписи `strings.editor.scripts =
  "Scripts"`, `strings.editor.settings = "Settings"`; счётчики — число непустых (после `trim`) скриптов и число
  настроек не `null`; разделы создаются при первом показе, как остальные.
- `ScriptsEditor`: переключатель из двух вкладок (`role="tablist"`) «Pre-request» / «Tests», под ним один
  `CodeEditor language="javascript"`; над редактором строка `strings.scripts.note` = «Скрипты пока не выполняются:
  выполнение появится на следующем этапе». Tab уводит фокус из редактора (как в теле).
- `RequestSettingsEditor`: четыре строки «название — выбор — пояснение»: «Следовать редиректам» (по умолчанию: да),
  «Максимум редиректов» (поле, пусто = по умолчанию 10), «Проверять SSL-сертификат» (по умолчанию — как в настройках
  приложения), «Отправлять тело у GET, HEAD, COPY, PURGE, UNLOCK» (`disable_body_pruning`, по умолчанию: нет). У трёх
  булевых — `<select class="select">` «По умолчанию / Да / Нет» ↔ `null / true / false`. Поле максимума: пусто →
  `null`; `^\d{1,4}$` → число; иное — `aria-invalid="true"`, подсказка «Целое число от 0 до 9999», черновик не
  меняется.
- `CodeEditor`: язык `javascript` подгружается отдельным динамическим импортом при первом таком редакторе;
  `BodyEditor` для raw с языком JavaScript передаёт `language="javascript"`.
- Конец строки: текст скрипта в черновик попадает как пришёл; пока пользователь его не правил, вкладка не
  «изменена».

- [ ] **Step 1: Падающие тесты.** `tabs.test.ts`: `normalizeContent("request", { method: "GET", url: "" })` даёт
  `scripts: { pre_request: "", test: "" }` и `settings` из четырёх `null`; повторная нормализация ничего не меняет;
  запрос со скриптом с `\r\n` после открытия не `dirty`. `RequestEditor.test.ts`: шесть разделов в порядке; у Scripts
  счётчик `1` при одном непустом скрипте, у Settings `2` при двух заданных; правка pre-request → вкладка `dirty`,
  Ctrl+S вызывает `saveNodeContent` с `scripts.pre_request`; переключение «Tests» показывает второй текст;
  `select` «Нет» у редиректов → `settings.follow_redirects === false`, «По умолчанию» → `null`, вкладка снова не
  `dirty`; максимум `3` → `3`, `1.5` и `-1` → `aria-invalid`, черновик прежний; отправка уходит с черновиком,
  содержащим настройки; редактор скрипта получает `data-language="javascript"`.
  `core_data.rs`: `nulls_and_empty_scripts_from_the_window_are_defaults` (`save_node_content` с `settings` из
  четырёх `null` и пустыми скриптами → `get_node` не содержит ключей `settings` и `scripts`).
- [ ] **Step 2: Запустить** `npm run test -- src/components/RequestEditor.test.ts src/lib/tabs.test.ts` — падают.
- [ ] **Step 3: Реализовать**; `npm install @codemirror/lang-javascript@6.2.5 --save-exact` (перед этим сверить
  `engines`).
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`; `cargo test -p requestmaster --test core_data`** —
  зелёные; записать прирост размера JavaScript сборки.
- [ ] **Step 5: Commit** — «План 2: вкладки Scripts и Settings запроса».

---

### Task 5: Редактор папки и Scripts у коллекции

**Files:**
- Modify: `ui/src/lib/types.ts`, `ui/src/lib/tabs.svelte.ts`, `ui/src/App.svelte`,
  `ui/src/components/CollectionEditor.svelte`, `ui/src/components/TreeMenu.svelte`, `ui/src/components/Tree.svelte`,
  `ui/src/lib/strings.ts`
- Create: `ui/src/components/FolderEditor.svelte`, `ui/src/components/FolderEditor.test.ts`
- Test: `ui/src/lib/tabs.test.ts`, `ui/src/components/CollectionEditor.test.ts`,
  `ui/src/components/TreeActions.test.ts`

**Interfaces:**
- Consumes: `ScriptsEditor`, `ScriptsDraft` (Task 4); `AuthEditor { auth, inherit }`.
- Produces:

```ts
export interface FolderDraft extends FolderContent { auth: Auth | null; scripts: ScriptsDraft }
export function normalizeContent(kind: "folder", content: FolderContent): FolderDraft;
export type TabKind = "request" | "collection" | "folder" | "environment" | "history";
export type TabContent = RequestDraft | CollectionDraft | FolderDraft;   // Tab.kind: "request" | "collection" | "folder"
export type TreeAction = "open" | "new_request" | "new_folder" | "rename" | "duplicate" | "export" | "delete";
FolderEditor: { tab: Tab<FolderDraft> }
```

**Поведение:**
- `createTab` для `folder` создаёт вкладку; `Tabs.open` открывает папку. Щелчок и Enter по папке в дереве
  по-прежнему только сворачивают и разворачивают. В меню строки папки первым пунктом — «Открыть»
  (`strings.tree.open`, значок `folder`): `workspace.select(id)` и `workspace.onOpen(id)`.
- `FolderEditor`: разделы Auth (`inherit` — `true`: у папки есть родитель) и Scripts. `CollectionEditor`: разделы
  Variables, Auth, Scripts (счётчик Scripts — как в Task 4). В `App.svelte` ветка `{:else}` заменена явными ветками
  по `tab.kind`.
- Удаление, переименование, отметка «не сохранено» в дереве, вопрос при экспорте коллекции учитывают вкладку
  папки без отдельного кода (работают по `nodeId`).

- [ ] **Step 1: Падающие тесты.** `tabs.test.ts`: тест «does not open a folder» переписан — папка открывается
  вкладкой `kind === "folder"`, черновик `{ auth: null, scripts: { pre_request: "", test: "" } }`.
  `FolderEditor.test.ts`: выбор Basic и возврат «Наследовать от родителя» → вкладка не `dirty`; «Без авторизации»
  сохраняется как `{ type: "no_auth" }`; правка скрипта → Ctrl+S → `saveNodeContent(id, …)` со скриптом.
  `CollectionEditor.test.ts`: раздел Scripts есть, правка сохраняется. `TreeActions.test.ts`: список пунктов меню
  папки начинается с «Открыть»; пункт открывает вкладку папки; щелчок по папке вкладку не открывает; удаление папки
  закрывает её вкладку; переименование меняет имя вкладки.
- [ ] **Step 2: Запустить** перечисленные файлы тестов — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «План 2: редактор папки и скрипты коллекции».

---

### Task 6: Дерево — вынос действий и виртуализация

**Files:**
- Create: `ui/src/lib/treeActions.svelte.ts`, `ui/src/components/TreeDialogs.svelte`, `ui/src/lib/exporting.ts`,
  `ui/src/lib/testing/bigTree.ts`, `ui/src/components/TreeVirtual.test.ts`
- Modify: `ui/src/components/Tree.svelte`, `TreeRow.svelte`, `Sidebar.svelte`, `EnvironmentList.svelte`,
  `ui/src/lib/tree.ts`, `ui/src/lib/workspace.svelte.ts`
- Test: `ui/src/lib/tree.test.ts`; существующие `Sidebar.test.ts`, `TreeActions.test.ts`, `Export.test.ts` проходят
  без правок

**Interfaces:**
- Produces:

```ts
// lib/tree.ts
export const ROW_HEIGHT = 30;      // px, equals --row-height of tokens.css
export const OVERSCAN = 8;         // rows drawn beyond each edge of the viewport
export interface VisibleRow { node: TreeNode; depth: number; posInSet: number; setSize: number }   // among shown siblings
/** Rows to draw: [first, last). A viewport of no height (no layout) means all rows. */
export function windowRange(scrollTop: number, viewportHeight: number, count: number): { first: number; last: number };
/** The scroll position that makes the row fully visible; unchanged when it already is. */
export function revealScrollTop(index: number, scrollTop: number, viewportHeight: number): number;
/** Index of the first row that lies wholly inside the viewport (not an overscan row); 0 without a layout or rows. */
export function firstVisibleIndex(scrollTop: number, viewportHeight: number, count: number): number;
// lib/treeActions.svelte.ts — the menu actions, the delete question and the export question of the tree
export class TreeActions { deleting: TreeNode | null; exporting: TreeNode | null; constructor(workspace: Workspace);
  run(api: Api, node: TreeNode, action: TreeAction): Promise<string | null>;   // id of the row to focus, if any
  confirmDelete(api: Api): Promise<string[] | null>; confirmExport(api: Api, save: boolean): Promise<void>;
  unsavedTabs(collectionId: string): AnyTab[] }
// lib/exporting.ts — «Сохранено: путь» / ничего при null / уведомление об ошибке (закрывает T-t)
export async function exportToFile(run: () => Promise<string | null>): Promise<void>;
// lib/testing/bigTree.ts
export function bigTree(folders: number, requestsPerFolder: number): NodeDto[];   // one collection
```

**Поведение:**
- Вынос: меню, удаление и экспорт уходят из `Tree.svelte` в `TreeActions` и `TreeDialogs.svelte`; `Tree.svelte`
  оставляет строки, клавиатуру и якорь меню. `EnvironmentList.svelte` и `TreeActions` пользуются `exportToFile`.
  Поведение не меняется.
- Прокрутка переезжает внутрь `Tree`: корень `div.tree-scroll` (`overflow: auto`), в нём распорка высотой
  `rows.length * ROW_HEIGHT` и строки, поставленные абсолютно (`style:top`); `.content` в `Sidebar.svelte` для раздела
  коллекций больше не прокручивается. Высота области читается при монтировании, при `scroll` и через
  `ResizeObserver`, если он есть в среде.
- Рисуются строки `windowRange(…)` и всегда — строка с фокусом (`workspace.focusedId`), строка в режиме
  переименования и перетаскиваемая строка (Task 7 дополняет набор). У строки `data-id`, `aria-posinset`,
  `aria-setsize`.
- Строка, на которой останавливается Tab (`tabbableId`, сейчас — «строка с фокусом, иначе первая»): строка с
  `workspace.focusedId`, если она есть среди показанных `rows` (она всегда отрисована); иначе
  `rows[firstVisibleIndex(scrollTop, высота области, rows.length)]`. `firstVisibleIndex` — это `Math.min(Math.ceil(
  scrollTop / ROW_HEIGHT), count - 1)`, не меньше `0`; при области без высоты — `0`. Брать `first` из `windowRange`
  нельзя: это строка из запаса `OVERSCAN` выше области, и Tab на неё дёрнул бы прокрутку назад. Правило покрывает
  три случая: фокуса в дереве ещё не было (`focusedId === null`), он обнулён после удаления узла, строка с фокусом
  скрыта фильтром или сворачиванием. В DOM всегда ровно одна строка с `tabindex="0"` (пока строки есть).
- `focusRow(index)`: `scrollTop = revealScrollTop(…)`, `tick()`, фокус по `[data-id]`. Прокрутка закрывает открытое
  меню строки (его якорь может исчезнуть).
- `Workspace.subtree(id)` строит индекс «родитель → дети» один раз за вызов (было: `rows.filter` на каждом уровне).

- [ ] **Step 1: Падающие тесты.** `tree.test.ts`: `windowRange(0, 300, 3000)` → `{ first: 0, last: 18 }`;
  `windowRange(3000, 300, 3000)` → `{ first: 92, last: 118 }`; `windowRange(89970, 300, 3000)` → `{ first: 2991,
  last: 3000 }`; `windowRange(0, 0, 3000)` → `{ first: 0, last: 3000 }`; `revealScrollTop(0, 600, 300)` → `0`;
  `revealScrollTop(50, 0, 300)` → `1230`; `revealScrollTop(5, 0, 300)` → `0`; `firstVisibleIndex(3000, 300, 3000)`
  → `100`; `firstVisibleIndex(3010, 300, 3000)` → `101`; `firstVisibleIndex(0, 300, 3000)` → `0`;
  `firstVisibleIndex(3000, 0, 3000)` → `0`; `firstVisibleIndex(95000, 300, 3000)` → `2999`; `firstVisibleIndex(0,
  300, 0)` → `0`; `visibleRows` отдаёт `posInSet` /
  `setSize` (вторая из трёх папок коллекции — `2` и `3`); `ROW_HEIGHT` равен числу из `--row-height` в
  `tokens.css?raw`. `TreeVirtual.test.ts` (`bigTree(30, 100)`, всё развёрнуто, `clientHeight` области подменён на
  `300`): в DOM не больше 30 элементов `treeitem`; высота распорки `90930px` (3031 строка); после `scrollTop = 3000`
  и события `scroll` строка с индексом 100 есть, строки с индексом 0 нет; строка с фокусом после прокрутки остаётся
  в DOM и остаётся `document.activeElement`; строка в режиме переименования после прокрутки сохраняет поле с
  введённым текстом; ArrowDown на последней отрисованной строке переводит фокус на следующую; фильтр по имени
  одного запроса оставляет три строки; прокрутка закрывает меню строки; без подмены высоты рисуются все строки.
  Остановка Tab: без фокуса в дереве при `scrollTop = 0` в DOM ровно один `treeitem` с `tabindex="0"` — строка с
  индексом 0 (прежнее поведение); без фокуса после `scrollTop = 3000` и события `scroll` — ровно один, и это
  строка с индексом 100 в списке строк; после фокуса на строке и прокрутки — ровно один, на строке с фокусом.
- [ ] **Step 2: Запустить** `npm run test -- src/lib/tree.test.ts src/components/TreeVirtual.test.ts` — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные; существующие тесты дерева не менялись.
- [ ] **Step 5: Commit** — «План 2: дерево — вынос действий и отрисовка только видимых строк».

---

### Task 7: Дерево — перемещение мышью и с клавиатуры

**Files:**
- Create: `ui/src/lib/treeDrop.ts`, `ui/src/lib/treeDrop.test.ts`, `ui/src/components/TreeDrag.test.ts`
- Modify: `ui/src/lib/api.ts`, `ui/src/lib/testing/fakeApi.ts`, `ui/src/lib/workspace.svelte.ts`,
  `ui/src/components/Tree.svelte`, `TreeRow.svelte`, `Sidebar.svelte`, `ui/src/lib/strings.ts`
- Test: `ui/src/components/Sidebar.test.ts` (перетаскивание файлов), `ui/src/components/Environments.test.ts`

**Interfaces:**
- Consumes: команда `move_node` (Task 1: `index` — позиция среди детей нового родителя без перемещаемого узла);
  `ROW_HEIGHT`, набор «всегда отрисованных» строк (Task 6).
- Produces:

```ts
// lib/api.ts
moveNode(id: string, parentId: string | null, index: number): Promise<void>;
// lib/treeDrop.ts — pure
export const NODE_DRAG_TYPE = "application/x-rqm-node";
export type DropZone = "before" | "inside" | "after";
export interface Place { parentId: string | null; index: number }
/** Zone of the row under the pointer; `null` — the dragged kind cannot be dropped on this row at all. */
export function dropZone(offsetY: number, over: NodeKind, dragged: NodeKind): DropZone | null;
export function descendantIds(rows: TreeRow[], id: string): Set<string>;     // without `id`
/** Where the drop puts the node; `null` — forbidden or nothing would change. */
export function dropPlace(rows: TreeRow[], draggedId: string, overId: string, zone: DropZone, overExpanded: boolean): Place | null;
export function keyboardPlace(rows: TreeRow[], id: string, key: "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"): Place | null;
/** The rows after the move: `parent_id` of the node and `sort_order` 0..n of the old and the new siblings. */
export function applyMove(rows: TreeRow[], id: string, place: Place): TreeRow[];
// lib/workspace.svelte.ts
async moveNode(api: Api, id: string, place: Place): Promise<boolean>;
```

**Правила:**
- `dropZone` (строка 30 px): над запросом — `offsetY < 15` → `before`, иначе `after`; над папкой — `< 8` → `before`,
  `>= 22` → `after`, иначе `inside`; над коллекцией: тащат коллекцию — `< 15` → `before`, иначе `after`; тащат папку
  или запрос — всегда `inside`; коллекцию над папкой или запросом — `null`.
- `dropPlace`: `null`, если цель — сам узел или его потомок, либо место не меняется. `inside` → последним ребёнком
  цели. `after` на развёрнутой папке с детьми → первым ребёнком этой папки. Иначе — рядом с целью у её родителя:
  позиция цели среди соседей без перемещаемого узла (`+1` для `after`).
- `keyboardPlace`: ↑/↓ — на одну позицию среди соседей (`null` на краю); → — последним ребёнком предыдущего соседа,
  если он папка; ← — к родителю родителя, сразу после родителя (`null`, если родитель — коллекция); для коллекции
  — только ↑/↓.
- `Workspace.moveNode`: сразу `rows = applyMove(…)` и `expanded.add(parentId)`; затем `api.moveNode`; отказ —
  уведомление и `reloadTree`; перемещения выполняются по очереди; возвращает, удалось ли.
- Перетаскивание: строка `draggable`, когда нет фильтра и она не в режиме переименования; `dragstart` кладёт id под
  `NODE_DRAG_TYPE`, `effectAllowed = "move"`, потомки считаются один раз; над допустимой целью — `preventDefault`,
  `dropEffect = "move"` и класс строки `drop-before` / `drop-inside` / `drop-after` (линия или рамка цветом
  `--accent`, без перехода); над недопустимой — ничего (ни `preventDefault`, ни класса). Зона считается от верха
  строки: `dropZone(event.clientY - row.getBoundingClientRect().top, …)`. После броска фокус — на перенесённой
  строке; `selectedId` не меняется; открытые вкладки и черновики не трогаются.
- `Sidebar.svelte`, обработчики на `window`. Сейчас `onDragOver` при любом перетаскивании вызывает
  `preventDefault` и пишет `dropEffect` (`"copy"` внутри панели, `"none"` снаружи). Событие всплывает от строки
  дерева к `window`, поэтому он перезаписал бы `dropEffect = "move"`, выставленный строкой, а при `effectAllowed =
  "move"` значение `"copy"` или `"none"` для WebView2 означает «бросок запрещён» — `drop` не придёт. Новое правило
  `onDragOver` — три ветки по `event.dataTransfer?.types ?? []`:
  1. есть `"Files"` — как раньше: `preventDefault`, `dropEffect` = `"copy"` внутри панели, `"none"` снаружи;
  2. есть `NODE_DRAG_TYPE` — обработчик сразу выходит: не вызывает `preventDefault` и не пишет `dropEffect`.
     Разрешение броска и `dropEffect = "move"` целиком решает строка дерева; над недопустимым местом бросок запрещён
     самим браузером;
  3. иное (текст, ссылка из другой программы) — `preventDefault`, как раньше (иначе WebView2 перешёл бы по
     брошенной ссылке и интерфейс пропал бы), и `dropEffect = "none"` везде (раньше внутри панели было `"copy"`,
     хотя импортировать там нечего).
  `onDrop` по-прежнему всегда вызывает `preventDefault`, а `importFiles` — только когда в `types` есть `"Files"` и
  бросок внутри панели.
- Клавиатура: Alt+стрелка на строке → `keyboardPlace` → `moveNode`; фокус остаётся на строке. Под фильтром —
  ничего.
- `fakeApi.moveNode` переставляет элементы массива узлов и отказывает по тем же правилам и с теми же текстами, что
  ядро.

- [ ] **Step 1: Падающие тесты.** `treeDrop.test.ts` на дереве `C1[ F1[ R1, R2 ], F2[], R3 ], C2[]`: `dropZone` — по
  значению на каждую ветку правил; `dropPlace`: R2 `before` R1 → `{ F1, 0 }`; R1 `after` R2 → `{ F1, 1 }`; R1
  `before` R2 → `null`; R3 `inside` F2 → `{ F2, 0 }`; R3 `inside` F1 → `{ F1, 2 }`; R3 `after` F1 (развёрнута) →
  `{ F1, 0 }`; R3 `after` F1 (свёрнута) → `{ C1, 1 }`; F1 `inside` F2 → `{ F2, 0 }`; F1 над R1 → `null`; F1 над F1 →
  `null`; C2 `before` C1 → `{ null, 0 }`; C1 `after` C2 → `{ null, 1 }`; R3 `inside` C2 → `{ C2, 0 }`.
  `keyboardPlace`: R1 ↓ → `{ F1, 1 }`; R1 ↑ → `null`; R3 → → `{ F2, 0 }`; R1 ← → `{ C1, 1 }`; F1 ← → `null`; C2 ↑ →
  `{ null, 0 }`; C1 → → `null`. `applyMove`: порядок и `sort_order` обоих родителей.
  `TreeDrag.test.ts` отрисовывает `Sidebar` (его обработчики на `window` участвуют в проверке) с тем же деревом,
  всё развёрнуто. В jsdom проекта (28.1.0) нет `DragEvent` и `DataTransfer`: `createEvent.dragOver` создаёт обычный
  `Event`, и `clientY` из `eventInit` в него не попадает. Поэтому координата задаётся явно помощником в самом файле
  теста (`test-setup.ts` не трогать):

  ```ts
  async function drag(type: "dragStart" | "dragOver" | "drop", target: Element, clientY: number, dataTransfer: object) {
    const event = createEvent[type](target, { dataTransfer });
    Object.defineProperty(event, "clientY", { value: clientY });
    await fireEvent(target, event);
    return event;
  }
  ```

  `dataTransfer` — обычный объект с `types`, `setData`, `getData`, `effectAllowed`, `dropEffect`, `files` (событие
  несёт этот же объект, тест читает из него `dropEffect`). `getBoundingClientRect` любой строки подменён: `top =
  100`, `height = 30`. Случаи (каждый — отдельный тест):
  - зоны, с разными ожидаемыми вызовами: R1 над F2, `clientY = 115` → у F2 класс `drop-inside`, бросок →
    `moveNode("R1", "F2", 0)`, строка R1 под F2, F2 развёрнута, фокус на R1; R1 над F2, `clientY = 103` → класс
    `drop-before`, бросок → `moveNode("R1", "C1", 1)`; R1 над F2, `clientY = 126` → класс `drop-after`, бросок →
    `moveNode("R1", "C1", 2)`; R3 над R1, `clientY = 120` → класс `drop-after`, бросок → `moveNode("R3", "F1", 1)`;
  - `dropEffect`: после `dragOver` узла R1 над F2 (`clientY = 115`; событие всплыло до `window`)
    `dataTransfer.dropEffect === "move"` и `event.defaultPrevented === true`; F1 над своим потомком R1 —
    `defaultPrevented === false`, класса цели нет, бросок ничего не вызывает; `types: ["Files"]` над панелью —
    `dropEffect === "copy"`, `defaultPrevented === true`, класса цели нет, бросок импортирует, `moveNode` не вызван;
    `types: ["text/uri-list"]` над панелью — `defaultPrevented === true`, `dropEffect === "none"`;
  - под фильтром `draggable === false`; строка в режиме переименования не `draggable`; отказ `moveNode` →
    уведомление и прежний порядок; узел, брошенный мимо строк, `importFiles` не вызывает; Alt+ArrowDown на R1 →
    `moveNode("R1", "F1", 1)`; Alt+ArrowRight на R3 → `moveNode("R3", "F2", 0)`; запрос с несохранённой вкладкой
    после переноса сохраняет вкладку, черновик и точку; два переноса подряд уходят по очереди; в большом дереве
    (`bigTree(30, 100)`, высота области 300) перетаскиваемая строка после прокрутки остаётся в DOM.

  `Sidebar.test.ts`: в двух существующих тестах перетаскивания файлов («cancels a drop and a dragover outside the
  sidebar…», «imports the files dropped on the sidebar») объект `dataTransfer` получает `types: ["Files"]`, как у
  настоящего перетаскивания файла; ожидания не меняются (правку назвать в отчёте).
- [ ] **Step 2: Запустить** оба новых файла — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «План 2: перемещение узлов дерева мышью и с клавиатуры».

---

### Task 8: Переменные в полях ввода — подсветка, подсказка, автодополнение, счётчик

**Files:**
- Create: `ui/src/lib/variables.ts`, `ui/src/lib/variables.test.ts`, `ui/src/lib/variableScope.svelte.ts`,
  `ui/src/components/VarInput.svelte`, `ui/src/components/VarTooltip.svelte`, `ui/src/components/VarSuggest.svelte`,
  `ui/src/components/Variables.test.ts`
- Modify: `ui/src/lib/api.ts`, `types.ts`, `testing/fakeApi.ts`, `strings.ts`, `workspace.svelte.ts`,
  `tabs.svelte.ts`, `ui/src/styles/tokens.css`, `ui/src/styles/contrast.test.ts`,
  `ui/src/components/ui/Field.svelte`, `KeyValueTable.svelte`, `UrlBar.svelte`, `RequestEditor.svelte`,
  `BodyEditor.svelte`, `EnvironmentEditor.svelte`

**Interfaces:**
- Consumes: команда `variables_in_scope` (Task 1); `Workspace.moveNode` (Task 7).
- Produces:

```ts
// lib/types.ts
export type VariableScopeName = "global" | "collection" | "environment" | "dynamic";
export interface ScopeVariable { name: string; scope: VariableScopeName; value: string | null; secret: boolean }
// lib/api.ts
variablesInScope(nodeId: string): Promise<ScopeVariable[]>;
// lib/variables.ts — pure
export interface VarToken { from: number; to: number; name: string }          // UTF-16 offsets, `to` exclusive
/** The `{{name}}` tokens the core replaces in its first pass: innermost, left to right. */
export function findTokens(text: string): VarToken[];
export function completionAt(text: string, caret: number): { from: number; prefix: string } | null;
export function applyCompletion(text: string, caret: number, from: number, name: string): { text: string; caret: number };
export function suggest(variables: ScopeVariable[], prefix: string): ScopeVariable[];   // at most 50
/** Names used where the core substitutes for this request: enabled rows only. */
export function requestTokenNames(draft: RequestDraft): string[];
export function describeVariable(name: string, variable: ScopeVariable | null): { scope: string; value: string };
// lib/variableScope.svelte.ts
export function bumpVariables(): void;                       // something that changes variables has happened
export class VariableScope { constructor(nodeId: string); variables: ScopeVariable[];
  lookup(name: string): ScopeVariable | null; refresh(api: Api): Promise<void>; readonly stale: boolean }
// context key "variables" → VariableScope; components
VarInput: Omit<HTMLInputAttributes, "value"> & { value: string }     // $bindable; a drop-in for <input type="text">
Field: + variables?: boolean                                          // default false
KeyValueTable: + variables?: boolean | "value"                        // default false; true — keys and values,
                                                                      // "value" — values only (path variables)
UrlBar: + unresolved: string[]
```

**Правила:**
- `findTokens` повторяет правило ядра (`rqm-vars`, `Resolver::pass`): токен начинается с `{{`, имя — до первой `{`
  или `}`, токен закрыт, только если сразу за именем `}}`; иначе сдвиг на один символ. Имя — любые символы, включая
  пробелы и кириллицу; пустое имя — тоже токен.
- `requestTokenNames`: URL; ключи и значения включённых строк query и headers; значения включённых path-переменных;
  тело — текст raw, ключи и значения включённых строк urlencoded, ключ и текст включённых текстовых частей
  form-data, `query` и `variables` GraphQL; поля собственного auth (Basic — оба, Bearer — токен, OAuth2 — токен и
  префикс, API Key — ключ и значение). Без повторов, в порядке появления. Метод, выключенные строки, пути файлов —
  нет.
- `VarInput`: под полем — слой-двойник (`div.var-layer`, `aria-hidden="true"`) с тем же текстом и теми же шрифтом,
  отступами и горизонтальной прокруткой; текст поля прозрачен, курсор и выделение видны. Токен в слое — `span` с классами `var
  var-global | var-collection | var-environment | var-dynamic | var-unresolved` (у ненайденной ещё и
  подчёркивание). Без `VariableScope` в контексте — обычное поле.
- Наведение мышью на токен → `VarTooltip` (`role="tooltip"`): имя, область словами, значение. Тексты
  (`strings.variables`): области — «Глобальная», «Коллекция», «Окружение», «Динамическая»; секрет — «••••••»;
  динамическая — «значение создаётся при отправке»; пустое — «пустое значение»; ненайденная — «Переменная не
  найдена».
- Автодополнение: набран `{{` + начало имени → `VarSuggest` (`role="listbox"`, фокус остаётся в поле,
  `aria-activedescendant`): имя, область, значение (секрет — маска). ↑/↓ — выбор, Enter и Tab — вставка `имя}}` без
  удвоения `}}`, Escape — закрыть. Ctrl+Space открывает список для токена под курсором.
- Где включено (каждое место — отдельная передача свойства, пропуск любой из них тесты Step 1 ловят): URL —
  `UrlBar` передаёт `variables` полю адреса; в `RequestEditor` таблица query — `variables` (ключи и значения),
  таблица Headers — `variables`, таблица path-переменных — `variables="value"` (ключ — обычное поле без слоя,
  значение подсвечено); в `BodyEditor` таблица urlencoded — `variables` (ключи и значения). Таблица переменных
  коллекции (`CollectionEditor`) `variables` не получает. Замаскированные ячейки (`type="password"`) — обычные поля
  без слоя; после «Показать» ячейка становится `VarInput`.
- `RequestEditor` создаёт `VariableScope(tab.nodeId)`, кладёт в контекст и считает `unresolved` = имена из
  `requestTokenNames(tab.draft)`, для которых `lookup` пуст; пока список переменных не загружен — пусто. `UrlBar`
  рядом с кнопкой Send показывает число (`role="status"`, текст для читалки «Не найдено переменных: N», подсказка —
  имена через запятую); доступное имя кнопки Send не меняется; отправка не блокируется.
- Обновление: `VariableScope` перечитывает переменные, когда её вкладка показана и после последнего чтения был
  `bumpVariables()`; берётся только последний ответ; отказ команды — одно уведомление, токены без цвета области.
  `bumpVariables()` вызывают: `Workspace.load`, `reloadEnvironments`, `setEnvironment` (когда команда завершилась),
  `deleteEnvironment`, `moveNode` (успех); `EnvironmentTab` после каждого успешного сохранения и `reload`;
  `Tab.save` вкладки коллекции.
- Токены: `--var-global: #e5a93b; --var-collection: #a98bf5; --var-environment: #4cc38a; --var-dynamic: #6aa6f8`.
- Попутно (T-s): в `KeyValueTable` и `EnvironmentEditor` строка, у которой сняли и снова поставили «секрет»,
  показывается замаскированной.

- [ ] **Step 1: Падающие тесты.** `variables.test.ts`: `findTokens("{{a}}/{{b}}")` → `[{0,5,"a"},{6,11,"b"}]`;
  `"{{a{{b}}}}"` → `[{3,8,"b"}]`; `"{{}}"` → `[{0,4,""}]`; `"{{a}"` → `[]`; `"я{{имя}}"` → `[{1,8,"имя"}]`;
  `"{{{a}}}"` → `[{1,6,"a"}]`; `"{{ a b }}"` → имя `" a b "`. `completionAt("a{{to", 5)` → `{ from: 3, prefix:
  "to" }`; `completionAt("{{a}} b", 7)` → `null`; `applyCompletion("x={{to", 6, 4, "token")` → `{ text:
  "x={{token}}", caret: 11 }`; `applyCompletion("{{to}}", 4, 2, "token")` → `{ text: "{{token}}", caret: 9 }`.
  `suggest`: сначала имена с этим началом, затем содержащие; регистр не важен. `requestTokenNames`: выключенный
  заголовок не учтён; ключ path-переменной не учтён, значение учтено; путь файла form-data не учтён; токен Bearer
  учтён. `contrast.test.ts`: четыре `--var-*` на `--bg` не ниже 4,5.
  `Variables.test.ts` (fake `variablesInScope`): URL `{{host}}/x/{{nope}}` → в слое `span.var-environment` и
  `span.var-unresolved`, счётчик «Не найдено переменных: 1», кнопка Send находится по прежнему имени; наведение
  (подменён `getBoundingClientRect` токена) показывает подсказку с «Окружение» и значением; у секретной переменной
  (`value: null, secret: true`) — маска; ввод `{{h` открывает список, Enter вставляет `{{host}}`, Escape закрывает и
  текст не меняет; смена активного окружения → повторный вызов `variablesInScope` и новый класс токена; из двух
  ответов, пришедших не по порядку, применён последний; скрытая вкладка не перечитывает, показанная — перечитывает;
  отказ команды — одно уведомление, поле работает; у коллекции после снятия и возврата «секрет» значение скрыто.
  Проводка в таблицы — через отрисованный `RequestEditor` (переменная `host` из окружения): значение заголовка
  `{{host}}` и значение query-параметра `{{host}}` дают в слое своей ячейки `span.var-environment`; ключ заголовка
  и ключ query-параметра с `{{host}}` тоже подсвечены; у path-переменной ячейка ключа не имеет `.var-layer`, а
  значение `{{host}}` подсвечено; тело urlencoded — ключ и значение поля с `{{host}}` дают `span.var-environment`.
  Маска — прямой отрисовкой `KeyValueTable` с `variables`, `secret` и `VariableScope` в контексте `"variables"` (в
  `RequestEditor` секретных строк нет): замаскированная ячейка — `input[type="password"]` без `.var-layer`, после
  «Показать» — со слоем и `span.var-environment`; та же таблица без `variables` слоёв не имеет вовсе.
- [ ] **Step 2: Запустить** `npm run test -- src/lib/variables.test.ts src/components/Variables.test.ts` — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные; существующие поиски полей по роли и
  имени (`getByRole("textbox", { name })`) не менялись.
- [ ] **Step 5: Commit** — «План 2: переменные в полях — подсветка, подсказка, автодополнение, счётчик у Send».

---

### Task 9: Переменные в редакторе кода

**Files:**
- Modify: `ui/package.json` (+ lock), `ui/src/components/CodeEditor.svelte`,
  `ui/src/lib/testing/CodeEditorStub.svelte`, `ui/src/components/BodyEditor.svelte`
- Create: `ui/src/lib/codemirror/variables.ts`, `ui/src/lib/codemirror/variables.test.ts`
- Test: `ui/src/components/Variables.test.ts`, `ui/src/components/RequestEditor.test.ts`,
  `ui/src/components/ResponsePanel.test.ts`, `ui/src/components/History.test.ts`

**Interfaces:**
- Consumes: `findTokens`, `suggest`, `applyCompletion`, `describeVariable`, `VariableScope`, контекст `"variables"`
  (Task 8).
- Produces:

```ts
// CodeEditor / CodeEditorStub
variables?: boolean                       // default false: highlight, hover and completion of {{variables}}
// CodeEditorStub puts it on its textarea: data-variables={variables ? "true" : "false"}
// lib/codemirror/variables.ts
/** Mark decorations for the tokens of the text: `cm-var cm-var-<scope>` or `cm-var cm-var-unresolved`. */
export function variableMarks(text: string, lookup: (name: string) => ScopeVariable | null): { from: number; to: number; class: string }[];
/** Completion source: options after `{{`; `null` elsewhere. */
export function variableCompletions(read: () => ScopeVariable[]): (context: CompletionContext) => CompletionResult | null;
/** The extensions of an editor with `variables`; `read` gives the current variables. */
export function variableSupport(read: () => ScopeVariable[]): Extension;
```

**Поведение:**
- `@codemirror/view 6.43.15` и `@codemirror/autocomplete 6.20.4` — прямые зависимости (версии из lock-файла).
- `CodeEditor` с `variables` подключает `variableSupport` через отдельный `Compartment`; при изменении
  `scope.variables` декорации пересчитываются без пересоздания редактора. Цвет токена перекрывает цвет разбора JSON
  (в том числе красный `tags.invalid` у `{"id": {{id}}}`). Подсказка по наведению и список автодополнения показывают
  те же тексты, что `VarTooltip` и `VarSuggest`; вставка не удваивает `}}`, которые поставил `closeBrackets`.
  Раскраска — только классами и токенами `tokens.css`, без атрибута `style`.
- Включено: тело raw в `BodyEditor` (передаёт редактору `variables`). Не включено: ответ (`ResponseBody`), история
  (`HistoryEntryView`), скрипты (`ScriptsEditor`) — там свойство не передаётся.
- Внешний вид в настоящем редакторе автотест не видит (в jsdom редактор заменён заглушкой) — шаги списка проверки.
  Передачу свойства автотест видит: заглушка отдаёт `data-variables`.

- [ ] **Step 1: Падающие тесты.** `lib/codemirror/variables.test.ts`: `variableMarks('{"id": {{id}}, "x":
  "{{nope}}"}', …)` — два диапазона с классами `cm-var cm-var-collection` и `cm-var cm-var-unresolved` и точными
  границами; `variableCompletions` на `EditorState` с текстом `{{to` и курсором в конце (`new
  CompletionContext(state, 4, false)`) отдаёт варианты `token`, `tokenUrl` с `from: 2`; на тексте `abc` — `null`;
  секретная переменная в варианте имеет `detail` с маской, а не значение.
  Проводка (компонентные тесты; `CodeEditor` подменён заглушкой через `vi.mock`, как в `RequestEditor.test.ts`):
  `Variables.test.ts` — редактор тела raw в отрисованном `RequestEditor` имеет `data-variables="true"`;
  `RequestEditor.test.ts` — редактор скрипта (раздел Scripts, Task 4) имеет `data-variables="false"` (проверка
  стоит здесь, а не в Task 4: атрибут у заглушки появляется только в этой задаче); `ResponsePanel.test.ts` —
  редактор тела ответа имеет `data-variables="false"`; `History.test.ts` — редактор тела в просмотре записи истории
  имеет `data-variables="false"`.
- [ ] **Step 2: Запустить** `npm run test -- src/lib/codemirror/variables.test.ts src/components/Variables.test.ts
  src/components/RequestEditor.test.ts src/components/ResponsePanel.test.ts src/components/History.test.ts` —
  новые случаи падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «План 2: переменные в редакторе тела».

---

### Task 10: Запоминание между запусками

**Files:**
- Create: `ui/src/lib/session.svelte.ts`, `ui/src/lib/Session.test.ts`
- Modify: `ui/src/lib/api.ts`, `types.ts`, `testing/fakeApi.ts`, `layout.svelte.ts`, `layout.test.ts`,
  `tabs.svelte.ts`, `tabs.test.ts`, `hotkeys.ts` (только комментарии), `strings.ts`, `ui/src/main.ts`,
  `ui/src/App.svelte`, `ui/src/App.test.ts`
- Test: `ui/src/components/Environments.test.ts`, `ui/src/components/History.test.ts` (закрытие окна)

**Interfaces:**
- Consumes: команды `load_session`, `save_window_state`, `save_draft` (Task 2); вид вкладки `folder` (Task 5).
- Produces:

```ts
// lib/types.ts
export interface SavedTab { kind: "node" | "environment"; id: string }
export interface WindowState { sidebar_width: number; placement: string; split: number; tabs: SavedTab[]; active: string | null }
export interface DraftDto { node_id: string; content: NodeContent }
export interface SessionDto { window: WindowState; drafts: DraftDto[] }
// lib/api.ts
loadSession(): Promise<SessionDto>; saveWindowState(state: WindowState): Promise<void>;
saveDraft(nodeId: string, content: NodeContent | null): Promise<void>;
// lib/layout.svelte.ts — без хранилища
class Layout { placement; split; apply(state: { placement?: unknown; split?: unknown }): void; choose(p): void; resize(s): void; side(w): Side }
// lib/tabs.svelte.ts
class Tab { constructor(node, content: C, draft?: C) }                 // draft — нормализованный черновик, если он есть
class Tabs { get activeId(): string | null;
  /** Opens the saved tabs in their order, one after another, without notifications; shows `active` at the end. */
  restore(api: Api, saved: SavedTab[], drafts: Map<string, NodeContent>, active: string | null): Promise<void> }
// lib/session.svelte.ts
export const WINDOW_SAVE_DELAY_MS = 500; export const DRAFT_SAVE_DELAY_MS = 1000;
export const SIDEBAR_DEFAULT = 280; export const SIDEBAR_MIN = 200; export const SIDEBAR_MAX = 480;
export class Session { sidebarWidth: number;
  start(api: Api): Promise<void>;     // reads the session and applies the layout; never rejects
  open(api: Api): Promise<void>;      // start() if not done; restores the tabs; begins to watch for changes
  flush(api: Api): Promise<boolean>;  // writes everything pending now; false — something was not written
  stop(): void }
export const session: Session;
```

**Поведение:**
- `main.ts`: `await session.start(tauriApi)` до `mount` — окно сразу рисуется в сохранённой раскладке.
  `App.svelte`: ширина боковой панели — `session.sidebarWidth`; в `onMount` — `await workspace.load(api)`, затем
  `await session.open(api)`. Отказ `loadSession` — уведомление и значения по умолчанию.
- Восстановление: вкладки в сохранённом порядке; узел или окружение, которые не читаются, пропускаются молча;
  `saved` — `normalizeContent(kind, содержимое из get_node)`, `draft` — `normalizeContent(kind, черновик)`; вкладка
  «изменена», только если они различаются. Ключ верхнего уровня, которого в сохранённом черновике нет (черновик
  записан сборкой, не знавшей этого поля), берётся из содержимого узла, а не из значения по умолчанию: черновик
  накладывается поверх `get_node` — `{ ...содержимое, ...черновик }` — и только потом нормализуется. Черновик узла,
  которого нет среди сохранённых вкладок, получает вкладку в конце. Активная вкладка ставится один раз; если
  сохранённой активной нет — первая. Панель ответа у восстановленной вкладки пуста. Значения вне границ (ширина
  200–480, доля 20–80, неизвестное расположение) приводятся как раньше в `Layout.load`.
- Негодный черновик. Черновик, который не удаётся наложить и нормализовать (`normalizeContent(kind, {
  ...содержимое, ...черновик })` бросает исключение: верный JSON не той формы, например `{ "headers": 5 }`, `{
  "body": { "mode": "url_encoded" } }`, у коллекции `{ "variables": 1 }`), отбрасывается: вкладка открывается с
  сохранённым содержимым узла и не «изменена», уведомления нет, ключ удаляется вызовом `api.saveDraft(id, null)`.
  Отказ этого удаления тоже молчаливый и восстановление не прерывает. `try/catch` охватывает только наложение и
  нормализацию черновика одной вкладки; чтение `get_node` и нормализация его содержимого остаются вне его. То же
  правило действует для осиротевшего черновика, который получает вкладку в конце. `restore` никогда не отклоняется
  из-за содержимого черновика, остальные вкладки восстанавливаются. Ограничение: ловится только то, что бросает
  при нормализации; значение, которое нормализуется без исключения, но с мусором (например `headers: [5]`), так не
  отсекается — полная проверка формы черновика в задачу не входит (решение № 39).
- Запись: состояние окна (порядок и вид вкладок без вкладок истории, активная, расположение, доля, ширина) —
  одним `saveWindowState` через 500 мс после последнего изменения. Черновик вкладки узла — `saveDraft(id, content)`
  через 1 с после последней правки, пока вкладка «изменена»; `saveDraft(id, null)` сразу, когда вкладка перестала
  быть «изменённой» (Ctrl+S, откат) или закрыта, — только если черновик этого узла записывался или был восстановлен.
  Отказ записи по таймеру — одно уведомление; следующая правка повторяет попытку.
- Закрытие окна (`onCloseRequested` в `App.svelte`): сначала `flush` вкладок окружений — по очереди и всех, даже
  если одно отказало (уведомление об отказе показывает сама вкладка, как сейчас); затем `session.flush(api)`. Всё
  записано → окно закрывается без вопроса, даже с «изменёнными» вкладками. Отказ любого из них — сохранения
  окружения, `saveDraft` или `saveWindowState` — ведёт к вопросу `strings.app.closeFailedTitle` = «Не удалось
  сохранить состояние. Закрыть всё равно?» с кнопками «Закрыть без сохранения» (`strings.app.closeWithoutSaving`)
  и «Отмена» (`strings.app.cancel`; фокус на «Отмена»), а не к молчаливому отказу закрыть окно: прежний `if
  (!saved) return false` удаляется. Вопрос показывается при первом же сбое (решение № 17; закрывает T-n для
  закрытия окна). «Закрыть без сохранения» закрывает окно; «Отмена» и Escape оставляют окно и вкладки с их
  правками (вкладка окружения остаётся открытой с несохранённым). Пока вопрос на экране, повторная просьба закрыть
  окно его не закрывает и второго вопроса не создаёт (как у прежнего вопроса).
- Закрытие **вкладки** не меняется: вопрос о сохранении у вкладки узла прежний; вкладка окружения, которое не
  сохраняется, по-прежнему остаётся открытой (`closeEnvironment`; T-n для вкладки остаётся отложенным).
- Удаляется как мёртвый код: в `layout.svelte.ts` — `STORAGE_KEY`, `load()`, `save()`, чтение в конструкторе и
  упоминание `localStorage` в комментарии модуля; в `App.svelte` — `layout.load()`, `dirtyTabs`, прежние `quitting`
  / `answerQuit`, константы `SIDEBAR_*`; в `strings.app` — `closeTitle`, `closeText`, `saveAllAndClose`
  (`closeWithoutSaving` и `cancel` остаются — их использует новый вопрос); тесты прежнего вопроса. После задачи
  поиск `rqm.layout` и `layout.load` по `ui/src` ничего не находит; поиск `localStorage` по `ui/src` ничего не
  находит вне `*.test.ts`, а в тестах слово встречается только в `layout.test.ts` — в одной проверке, что `choose`
  и `resize` в него не пишут (она страхует от возврата записи).

- [ ] **Step 1: Падающие тесты.** `layout.test.ts` переписан на `apply` (те же случаи терпимости: нет значения,
  неизвестное расположение → `auto`, доля вне 20–80 → граница, не число → 50) и «`choose` и `resize` ничего не
  пишут в `localStorage`» (после вызовов `localStorage.length === 0`). Из файла уходят `const KEY = "rqm.layout"`,
  все `localStorage.setItem/getItem/clear` прежних случаев и два теста о недоступном хранилище («works with the
  defaults when the storage cannot be read», «still applies a choice when the storage cannot be written») — читать
  и писать больше нечего. `tabs.test.ts`: `restore` открывает три вкладки (запрос, папка, окружение) в порядке и
  делает активной сохранённую; `getNode` вызывается по одному; отсутствующий узел пропущен без уведомления;
  вкладка с черновиком `dirty` и показывает адрес из черновика; черновик, равный сохранённому, не `dirty`; черновик
  старой формы (без `scripts`, `settings`) нормализуется, а ключ, которого в нём нет, но который есть у узла
  (`scripts` со скриптом), берётся от узла и не делает вкладку `dirty`; осиротевший черновик получает вкладку.
  Негодный черновик (`tabs.test.ts`): три сохранённые вкладки, у второй черновик `{ headers: 5 }` → открыты все три
  в сохранённом порядке, вторая не `dirty` и показывает сохранённый адрес узла, `toasts.items` пуст, в
  `api.draftCalls` есть `[id, null]`, активной стала сохранённая; то же с черновиком `{ body: { mode:
  "url_encoded" } }`; то же для вкладки коллекции с черновиком `{ variables: 1 }`; осиротевший черновик `{ headers:
  5 }` получает вкладку в конце, не `dirty`, ключ удалён; `saveDraft` отклоняется при удалении негодного черновика
  → `restore` всё равно завершается, все вкладки открыты, уведомлений нет.
  `Session.test.ts` (поддельные таймеры): правка URL → через 1000 мс один `saveDraft(id, content)`; серия правок —
  один вызов; Ctrl+S → `saveDraft(id, null)`; «Не сохранять» при закрытии вкладки → `saveDraft(id, null)`; вкладка
  без правок `saveDraft` не вызывает; пять нажатий стрелки на разделителе → один `saveWindowState` через 500 мс с
  новой `split`; открытие и закрытие вкладки меняет `tabs` в записанном состоянии; вкладка истории в него не
  попадает; `flush` пишет отложенное сразу; отказ `loadSession` → уведомление, раскладка по умолчанию.
  `App.test.ts`: в `afterEach` (сейчас строки 38–39) `localStorage.clear()` и `layout.load()` убраны, вместо них
  — `session.stop()`, `layout.apply({})`; закрытие окна с «изменённой» вкладкой и ожидающими записями →
  `saveDraft` и `saveWindowState` вызваны, окно закрывается без вопроса; отказ `saveDraft` при закрытии → вопрос
  `closeFailedTitle`, «Закрыть без сохранения» закрывает, «Отмена» оставляет окно; отказ `saveWindowState` при
  закрытии → тот же вопрос; вторая просьба закрыть окно при открытом вопросе → `false`, диалог один; сохранённая
  ширина 320 → боковая панель 320.
  `Environments.test.ts` (там уже есть подготовка: окружение с правкой и `saveEnvironment`, который отказывает):
  существующий тест «keeps the window open and notifies when the save before closing the window fails»
  переписывается — `saveEnvironment` отказывает при закрытии окна → уведомление вкладки и вопрос
  `closeFailedTitle` (а не сразу `false`); «Закрыть без сохранения» → `requestClose()` даёт `true`; в отдельном
  случае «Отмена» → `false`, вкладка окружения на месте и хранит правку. Существующий тест «asks about the drafts
  of requests only after the environments are saved» переписывается: окружение с правкой и запрос с правкой →
  `saveEnvironment` вызван один раз, затем `saveDraft`, вопроса нет, `requestClose()` даёт `true`. Тест «saves once
  and lets the window close when it is closed with a pending save» остаётся без правок. Закрытие **вкладки**
  окружения при отказе сохранения — существующий тест остаётся без правок (вкладка не закрывается).
  `History.test.ts`: тест «is not counted among the unsaved tabs when the window is closed» переписывается —
  вкладка истории и запрос с правкой → окно закрывается без вопроса, в записанном `saveWindowState` вкладки истории
  нет; тест «is never unsaved…» остаётся без правок. Все изменённые и удалённые утверждения назвать в отчёте.
- [ ] **Step 2: Запустить** `npm run test -- src/lib src/App.test.ts src/components/Environments.test.ts
  src/components/History.test.ts` — падают.
- [ ] **Step 3: Реализовать**; `fakeApi`: `FakeSeed.session?: SessionDto`, `FakeApi.windowStates: WindowState[]`,
  `FakeApi.draftCalls: [string, NodeContent | null][]`; по умолчанию — пустая сессия.
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные; результаты поиска мёртвого кода — в отчёт.
- [ ] **Step 5: Commit** — «План 2: вкладки, черновики и раскладка переживают перезапуск».

---

### Task 11: Окно настроек приложения

**Files:**
- Create: `ui/src/components/SettingsDialog.svelte`, `ui/src/components/Settings.test.ts`
- Modify: `ui/src/lib/api.ts`, `types.ts`, `testing/fakeApi.ts`, `icons.ts`, `strings.ts`, `ui/src/App.svelte`

**Interfaces:**
- Consumes: команды `get_settings`, `save_settings` (Task 2); `Dialog { title, onclose, children }`.
- Produces:

```ts
export type ProxyDto = { mode: "system" } | { mode: "off" } | { mode: "manual"; url: string };
export interface SettingsDto { timeout_ms: number; verify_tls: boolean; proxy: ProxyDto }
getSettings(): Promise<SettingsDto>; saveSettings(settings: SettingsDto): Promise<void>;   // lib/api.ts
SettingsDialog: { onclose: () => void }
// icons.ts: + "settings"
```

**Поведение:**
- Кнопка-значок «Настройки» (`strings.settings.button`) в строке вкладок после «Расположение ответа» открывает
  диалог «Настройки»; при открытии читается `getSettings`.
- Поля: «Таймаут запроса, мс» (число; пояснение «0 — без ограничения»); галочка «Проверять SSL-сертификаты»
  (пояснение «Запрос может переопределить это на вкладке Settings»); «Прокси» — три варианта: «Системный»
  (пояснение «PAC и WPAD не поддерживаются»), «Без прокси», «Вручную» с полем адреса `http://host:port`.
- «Сохранить»: таймаут — целое от 0 до 3 600 000, иначе под полем «Целое число от 0 до 3 600 000»; адрес при
  «Вручную» — начинается с `http://` или `https://` и не пуст после схемы, иначе «Адрес должен начинаться с http://
  или https://»; при ошибке команда не вызывается. Успех — диалог закрывается, уведомление «Настройки сохранены».
  Отказ команды — её текст показан в диалоге, диалог остаётся. «Отмена» и Escape — без сохранения.
- Настройки действуют со следующей отправки.

- [ ] **Step 1: Падающие тесты** `Settings.test.ts`: диалог открывается с сохранёнными значениями; `abc`, `-1`,
  `3600001` в таймауте → текст ошибки, `saveSettings` не вызван; `0` — допустимо; «Вручную» с `host:8080` → ошибка,
  вызова нет; верные значения → `saveSettings({ timeout_ms: 30000, verify_tls: false, proxy: { mode: "manual", url:
  "http://127.0.0.1:8888" } })`, диалог закрыт, уведомление; «Системный» → `proxy: { mode: "system" }` без `url`;
  отказ `saveSettings` → текст в диалоге, диалог открыт; Escape закрывает без вызова; у кнопки есть `aria-label`.
- [ ] **Step 2: Запустить** `npm run test -- src/components/Settings.test.ts` — падает.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «План 2: окно настроек приложения».

---

### Task 12: Тело form-data и binary

**Files:**
- Create: `ui/src/components/FormDataEditor.svelte`, `ui/src/components/BinaryBodyEditor.svelte`,
  `ui/src/components/BodyFiles.test.ts`
- Modify: `ui/src/lib/api.ts`, `types.ts`, `testing/fakeApi.ts`, `strings.ts`,
  `ui/src/components/BodyEditor.svelte`
- Test: `ui/src/components/RequestEditor.test.ts`, `ui/src/lib/tabs.test.ts`, `src-tauri/tests/core_send.rs`

**Interfaces:**
- Consumes: команда `pick_files` (Task 3); `VarInput` (Task 8).
- Produces:

```ts
// lib/api.ts
pickFiles(multiple: boolean): Promise<string[]>;                   // empty — the user closed the window
// lib/types.ts
export interface FileRow { path: string; confirmed: boolean }
export type FormValueRow = { text: string } | { files: FileRow[] };
export interface FormPartRow { key: string; value: FormValueRow; enabled: boolean; content_type: string | null; postman_extra: Extra }
// FormPart.content_type: `?: string | null`.  DraftBody: form_data → data: FormPartRow[]; binary → data: { file: FileRow | null }
FormDataEditor:   { parts: FormPartRow[] }        // $bindable
BinaryBodyEditor: { file: FileRow | null }        // $bindable
```

**Поведение:**
- `BodyEditor`: редактируются `none`, `raw`, `url_encoded`, `form_data`, `binary`; только для чтения остаётся
  GraphQL (пометка и вопрос «Заменить тело?» — прежние, теперь только для него). Память о введённом (`kept`) —
  для всех четырёх видов; она сбрасывается, когда тело заменено снаружи (откат «Не сохранять», восстановление) —
  отложенное W-l.
- `FormDataEditor` — таблица: «включено / ключ / вид (Текст | Файл) / значение / Content-Type / удалить» и пустая
  строка для добавления. Ключ (у части любого вида) и текст текстовой части — `VarInput` (подсветка, подсказка и
  автодополнение переменных, как в таблицах Task 8). У части-файла: список имён с полным путём в подсказке, у
  каждого крестик; кнопка «Выбрать файлы…» → `pickFiles(true)`, выбранные заменяют список и получают `confirmed:
  true`; отмена ничего не меняет. Неподтверждённый файл (из импорта) помечен «выберите заново»
  (`strings.body.fileUnconfirmed`). Несколько файлов в одной части сохраняются как есть. Смена вида «Текст ↔ Файл»
  сохраняет `key`, `enabled`, `content_type`, `postman_extra` и даёт пустое значение нового вида. Пустое поле
  Content-Type → `null`.
- `BinaryBodyEditor`: путь файла или «Файл не выбран», кнопки «Выбрать файл…» (`pickFiles(false)`) и «Убрать»; та
  же пометка неподтверждённого.
- Поля ручного ввода пути нет: `confirmed: true` появляется только у пути, который вернула `pickFiles`.

- [ ] **Step 1: Падающие тесты.** `tabs.test.ts`: нормализация form-data выписывает `content_type: null` и
  `confirmed: false`; binary — `confirmed`. `BodyFiles.test.ts`: добавить текстовую часть вводом ключа; переключить
  на «Файл» — `postman_extra` и ключ прежние, значение `{ files: [] }`; «Выбрать файлы…» с ответом двух путей → два
  файла `confirmed: true`; отмена (`[]`) ничего не меняет; импортированная часть с двумя неподтверждёнными файлами
  показывает оба и пометку; удаление одного файла; очистка Content-Type даёт `null`, вкладка без других правок не
  `dirty`; binary: выбор и «Убрать»; переключение видов и обратно возвращает введённое; после отката вкладки
  возвращается сохранённое, а не введённое. Подсветка (отрисовка через `RequestEditor` с fake `variablesInScope`,
  переменная `host` из окружения): ключ и текст текстовой части form-data с `{{host}}` дают в слое своей ячейки
  `span.var-environment`; у части-файла ячейка значения не имеет `.var-layer` (ключ части-файла подсвечен, как у
  текстовой). `RequestEditor.test.ts`: тест read-only form-data переписан на GraphQL.
  `core_send.rs`: `form_data_from_the_window_is_sent` (черновик с текстовой частью и частью-файлом `confirmed:
  true`, `content_type: null` → wiremock получает обе части).
- [ ] **Step 2: Запустить** новые и изменённые файлы — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`; `cargo test -p requestmaster --test core_send`** —
  зелёные.
- [ ] **Step 5: Commit** — «План 2: редактирование form-data и binary».

---

### Task 13: Ответ — «Сохранить в файл» и поиск

**Files:**
- Modify: `ui/package.json` (+ lock), `ui/src/lib/api.ts`, `testing/fakeApi.ts`, `icons.ts`, `strings.ts`,
  `hotkeys.ts`, `ui/src/App.svelte`, `ui/src/components/CodeEditor.svelte`,
  `ui/src/lib/testing/CodeEditorStub.svelte`, `ResponsePanel.svelte`, `ResponseBody.svelte`, `ResponseStatus.svelte`
- Test: `ui/src/components/ResponsePanel.test.ts`, `ui/src/App.test.ts`

**Interfaces:**
- Consumes: команда `save_response_body` (Task 3); `exportToFile` (Task 6).
- Produces:

```ts
saveResponseBody(bodyId: string): Promise<string | null>;        // lib/api.ts
interface HotkeyActions { …; find(): void }                      // Ctrl+F
ResponseStatus: + onsave: () => void                             // icons.ts: + "save"
ResponseBody:   + onsave: () => void;  export function openSearch(): void
ResponsePanel:  + findSignal?: number                            // a new value opens the search in the body
CodeEditor / CodeEditorStub: export function openSearch(): void  // the stub sets data-search-open on its textarea
```

**Поведение:**
- Кнопка-значок «Сохранить в файл» (`strings.response.save`) в строке статуса рядом с «Копировать» — у любого
  ответа с телом; в полосе «Показаны первые 256 КБ…» — ещё и текстовая кнопка «Сохранить в файл». Обе вызывают
  `saveResponseBody(body.id)` через `exportToFile`: путь → «Сохранено: путь»; `null` → ничего; отказ → уведомление.
  У вкладки истории кнопки нет.
- Ctrl+F (`hotkeys.ts`): если событие пришло изнутри `.cm-editor` — не перехватывается (поиск открывает сам
  редактор); иначе действие по умолчанию отменяется всегда (строка поиска WebView2 не появляется) и вызывается
  `find`. `find` в `App.svelte`: показанная вкладка — запрос с ответом, у которого есть тело → раздел Body и панель
  поиска в редакторе тела; иначе ничего. При открытом диалоге — ничего (общее правило).
- `@codemirror/search 6.7.2` — прямая зависимость; `openSearch` вызывает `openSearchPanel`. Тексты панели — на
  русском через `EditorState.phrases` из `strings.search` («Найти», «следующее», «предыдущее», «все», «с учётом
  регистра», «регулярное выражение», «слово целиком», «закрыть»; для редактируемых редакторов — «Заменить»,
  «заменить», «заменить все»). Поля и кнопки панели оформлены токенами в теме `CodeEditor`.

- [ ] **Step 1: Падающие тесты.** `ResponsePanel.test.ts`: «Сохранить в файл» вызывает `saveResponseBody` с id
  тела и показывает «Сохранено: C:\out\response.json»; `null` — без уведомлений; отказ — уведомление об ошибке; у
  ответа без тела кнопки нет; у обрезанного тела кнопка есть и в полосе; новое значение `findSignal` при разделе
  Headers переключает на Body и ставит `data-search-open`. `App.test.ts`: Ctrl+F на вкладке запроса с ответом
  открывает поиск; Ctrl+F из элемента внутри `.cm-editor` не отменён и `find` не вызван; Ctrl+F без ответа
  отменён и ничего не меняет; при открытом диалоге — ничего; вкладка истории кнопки «Сохранить в файл» не имеет.
- [ ] **Step 2: Запустить** оба файла — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «План 2: сохранение ответа в файл и поиск в ответе».

---

### Task 14: Мелкие исправления — полоса вкладок и окружение без имени

**Files:**
- Modify: `ui/src/components/TabBar.svelte`, `ui/src/App.svelte`, `ui/src/test-setup.ts`, `ui/src/lib/format.ts`,
  `ui/src/lib/format.test.ts`, `ui/src/lib/strings.ts`, `ui/src/lib/workspace.svelte.ts`,
  `ui/src/lib/tabs.svelte.ts`, `ui/src/components/ImportReport.svelte`
- Create: `ui/src/components/TabBar.test.ts`
- Test: `ui/src/components/Environments.test.ts`, `ui/src/components/Sidebar.test.ts`

**Interfaces:**
- Produces:

```ts
// lib/format.ts
/** New `scrollLeft` of the tab bar for a wheel turn; `null` — the bar does not scroll (nothing to prevent). */
export function wheelScrollLeft(deltaX: number, deltaY: number, bar: { scrollLeft: number; scrollWidth: number; clientWidth: number }): number | null;
/** The name to show and to save: `strings.environment.untitled` for a blank one. */
export function environmentName(name: string): string;
// strings.environment.untitled = "Без названия"
```

**Поведение:**
- Колесо над полосой вкладок: вертикальный поворот (`deltaX === 0`) двигает полосу по горизонтали; действие по
  умолчанию отменяется, только если полоса сдвинулась. Горизонтальный жест не трогается.
- Активная вкладка доводится до видимости при каждой её смене и при изменении числа вкладок: `scrollIntoView({
  block: "nearest", inline: "nearest" })` без плавности (спека: прокрутка не анимируется). В `test-setup.ts` —
  заглушка `Element.prototype.scrollIntoView` (в jsdom её нет).
- Рамка фокуса: нажатие указателя на заголовок вкладки ставит ему класс `by-pointer` (рамка скрыта); класс
  снимается при потере фокуса и при нажатии стрелки или Tab на вкладке — клавиатурная рамка остаётся.
- Попутно (W-o): заголовок вкладки получает `id` и `aria-controls`, панель вкладки в `App.svelte` —
  `role="tabpanel"`, `id`, `aria-labelledby`.
- Окружение с пустым (или из пробелов) именем: `environmentName` применяется к именам, пришедшим из ядра, в
  `Workspace.load` и `reloadEnvironments`, в конструкторе `EnvironmentTab`, в `ImportReport` (для окружения).
  Глобальные переменные не затрагиваются. Правка такого окружения сохраняется под именем «Без названия».

- [ ] **Step 1: Падающие тесты.** `format.test.ts`: `wheelScrollLeft(0, 100, { scrollLeft: 0, scrollWidth: 900,
  clientWidth: 300 })` → `100`; у правого края (`scrollLeft: 600`) → `null`; `wheelScrollLeft(40, 0, …)` → `null`;
  полоса без переполнения → `null`; `environmentName("  ")` → «Без названия», `environmentName("Dev")` → `Dev`.
  `TabBar.test.ts`: событие `wheel` на переполненной полосе (размеры подменены) меняет `scrollLeft` и отменено;
  открытие вкладки вызывает `scrollIntoView` у активного заголовка без `behavior: "smooth"`; после `pointerdown`
  заголовок имеет класс `by-pointer`, после ArrowRight у нового заголовка класса нет; заголовок и панель связаны
  `aria-controls` / `aria-labelledby`. `Environments.test.ts`: окружение с именем `""` — в списке и в
  переключателе «Без названия»; правка переменной → `saveEnvironment(id, "Без названия", …)` и сохранение
  проходит; вкладка и окно закрываются. `Sidebar.test.ts`: отчёт импорта окружения без имени — «Окружение: Без
  названия».
- [ ] **Step 2: Запустить** перечисленные файлы — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «План 2: колесо и рамка фокуса в полосе вкладок, окружение без имени».

---

### Task 15: Описание коллекции и папки (по решению владельца — см. «Решения автора», № 1)

**Files:**
- Create: `crates/rqm-postman/src/description.rs`
- Modify: `crates/rqm-postman/src/lib.rs`, `crates/rqm-postman/tests/export.rs`, `src-tauri/src/core.rs`
- Modify: `ui/src/lib/types.ts`, `strings.ts`, `ui/src/components/CollectionEditor.svelte`, `FolderEditor.svelte`
- Test: `src-tauri/tests/core_tree.rs`, `ui/src/components/CollectionEditor.test.ts`, `FolderEditor.test.ts`

**Interfaces:**
- Consumes: `FolderDraft`, `FolderEditor` (Task 5); `top_extra` в `core.rs`.
- Produces:

```rust
// crates/rqm-postman/src/description.rs, re-exported from lib.rs
/// The description kept in the Postman source object of a collection (`info.description`) or of a folder
/// (`description`); empty when there is none. A request has none here.
pub fn description_of(kind: NodeKind, extra: &Extra) -> String;
/// Writes the description into the source object; a text equal to the current one changes nothing.
pub fn set_description(kind: NodeKind, extra: &mut Extra, text: &str);
```

```ts
// CollectionContent, FolderContent: + description?: string;  CollectionDraft, FolderDraft: + description: string
```

**Правила:**
- Чтение: значение-строка — она сама; объект со строковым `content` — `content`; `null`, отсутствие, иное — пусто.
- Запись (P2): текст равен прочитанному — объект не меняется (остаются явный `null` и форма-объект). Иначе: была
  форма-объект — меняется только `content`; иначе пишется строка. Пустой текст при отсутствующем ключе ключ не
  создаёт. У коллекции без `info` в исходном объекте (создана в программе) `info` создаётся с новым `_postman_id` и
  схемой v2.1 — теми же, что ставит экспорт.
- `AppCore::get_node`: для коллекции и папки в `content` добавляется ключ `description` (только когда не пуст) —
  читается из верхнего `postman_extra` до его вырезания. `save_node_content`: ключ `description` вынимается из
  присланного JSON и применяется к сохранённому верхнему `postman_extra`; ключа нет — описание не трогается (пустая
  строка — это уже правка). Сам `postman_extra` в окно по-прежнему не уходит. Экспорт не меняется.
- Окно: раздел «Описание» (`strings.collection.description`) в редакторах коллекции и папки — `<textarea>` с
  исходным текстом (Markdown не отрисовывается); черновик, Ctrl+S, откат — как у остального содержимого.

- [ ] **Step 1: Падающие тесты.** Модульные в `description.rs`: три формы чтения; запись того же текста не меняет
  объект (`{"description": null}` остаётся); правка формы-объекта меняет только `content`; пустой текст не создаёт
  ключ. `export.rs`: импорт коллекции с описаниями у коллекции и папки → экспорт без правок — `semantic_diff` пуст.
  `core_tree.rs`: `description_round_trips_through_the_window` (get → правка → save → get; в экспорте новое
  описание, остальные ключи `info` на месте); `description_of_a_collection_created_here_exports_with_the_schema`;
  `content_without_the_description_key_keeps_the_stored_description`; `empty_description_from_the_window_clears_it`.
  Окно: правка описания делает вкладку
  `dirty`, Ctrl+S отправляет `content.description`, откат возвращает текст.
- [ ] **Step 2: Запустить** `cargo test -p rqm-postman -p requestmaster` и два файла тестов окна — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `cargo test --workspace`, clippy, `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «План 2: описание коллекции и папки».

---

### Task 16: Строгая CSP

**Files:**
- Modify: `src-tauri/tauri.conf.json`, `ui/index.html`, `ui/src/components/CodeEditor.svelte`
- Create: `ui/src/styles/csp.test.ts`
- Test: `src-tauri/tests/core_data.rs`

**Interfaces:**
- Consumes: все расширения CodeMirror предыдущих задач (подсветка JavaScript, переменные, поиск).
- Produces: `tauri.conf.json` — `"csp": "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'
  data:; font-src 'self'; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action
  'none'; frame-src 'none'; worker-src 'none'"`; ключ `dangerousDisableAssetCspModification` удалён.

**Поведение:**
- `ui/index.html` получает в `<head>` тег `<style>` с одним настоящим правилом (`html { color-scheme: dark; }`):
  Tauri при сборке ставит ему nonce и дописывает этот nonce в `style-src`. `CodeEditor` читает nonce свойством
  `.nonce` этого тега (`document.querySelector<HTMLStyleElement>("style[nonce]")?.nonce`) и передаёт редактору
  расширением `EditorView.cspNonce.of(nonce)`; пустой nonce — расширение не добавляется.
- После `npm run build` проверить, что в `ui/dist/index.html` тег `<style>` остался.
- В живом окне CSP автотестом не проверяется. Откат, если владелец увидит редактор без оформления: вернуть в
  `tauri.conf.json` `style-src 'self' 'unsafe-inline'` и `"dangerousDisableAssetCspModification": ["style-src"]`
  (записать в итогах и в списке проверки).

- [ ] **Step 1: Падающие тесты.** `core_data.rs`: `csp_is_strict` — в `tauri.conf.json` строка CSP не содержит
  `unsafe-inline` и `unsafe-eval`, содержит `style-src 'self'`, `frame-src 'none'`, `form-action 'none'`; ключа
  `dangerousDisableAssetCspModification` нет. `csp.test.ts`: ни в одном `*.svelte` из `ui/src`
  (`import.meta.glob("../**/*.svelte", { query: "?raw", eager: true })`) нет статического атрибута `style="`, нет
  `{@html`; в `index.html?raw` есть тег `<style>`.
- [ ] **Step 2: Запустить** `cargo test -p requestmaster --test core_data csp_is_strict` и `npm run test --
  src/styles/csp.test.ts` — падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `cargo test --workspace`, clippy, `npm run check`, `npm run test`, `npm run build`** — зелёные;
  `ui/dist/index.html` содержит `<style`.
- [ ] **Step 5: Commit** — «План 2: строгая CSP для стилей».

---

### Task 17: Режим замера и замеры раздела 10

**Files:**
- Create: `src-tauri/src/measure.rs`, `src-tauri/tests/core_perf.rs`, `src-tauri/tests/common/big_collection.rs`,
  `ui/src/lib/measure.ts`, `ui/src/lib/measure.test.ts`, `scripts/measure-start.ps1`
- Modify: `src-tauri/src/main.rs`, `lib.rs`, `core.rs`, `session.rs`, `dto.rs`, `commands.rs`, `build.rs`,
  `capabilities/default.json`; все литералы `CoreConfig` в `src-tauri/tests/*.rs`
- Modify: `ui/src/lib/api.ts`, `types.ts`, `testing/fakeApi.ts`, `session.svelte.ts`, `ui/src/App.svelte`,
  `ui/src/App.test.ts`, `ui/src/components/TabBar.svelte`, `UrlBar.svelte`, `Sidebar.svelte`, `BodyEditor.svelte`
  (в четырёх компонентах — только атрибуты `data-measure`)
- Test: `src-tauri/tests/core_data.rs`

**Interfaces:**
- Consumes: класс `Session` (`start`, `open`) и `FakeSeed.session` (Task 10).
- Produces:

```rust
pub struct CoreConfig { pub db_path, pub bodies_dir, pub history_keep, /// File the timings are appended to; `None` — not measuring.
                        pub measure_file: Option<PathBuf> }
impl AppCore { /// Appends `name<TAB>ms` lines to the measure file; without one does nothing.
               pub fn report_timings(&self, samples: Vec<TimingSample>) -> Result<(), String>; }
// dto.rs: TimingSample { name: String, ms: f64 };  SessionDto: + measure: bool
// tests/common/big_collection.rs
pub fn big_collection(folders: usize, requests_per_folder: usize) -> serde_json::Value;   // Postman v2.1, synthetic
```

```ts
// lib/types.ts — SessionDto: + measure: boolean (обязательный ключ, как отдаёт ядро)
export interface TimingSample { name: string; ms: number }
// lib/api.ts
reportTimings(samples: TimingSample[]): Promise<void>;
// lib/session.svelte.ts
class Session { readonly measure: boolean }   // from loadSession; false until start() and when loadSession fails
// lib/measure.ts
export const MEASURE_FLUSH_MS = 2000;
/** Starts the measuring; the returned function stops it (no more samples are collected or sent). */
export function installMeasure(api: Api): () => void;
// lib/testing/fakeApi.ts
// FakeSeed.session?: Omit<SessionDto, "measure"> & { measure?: boolean }   — createFakeApi подставляет measure: false
// FakeApi.timingCalls: TimingSample[][]                                      — каждый вызов reportTimings, по порядку
```

**Поведение:**
- `main.rs`: переменная окружения `RQM_MEASURE_DIR=<папка>` включает режим замера: база — `<папка>\app.db`, тела
  ответов — `<папка>\bodies` (общая временная папка тел не очищается, рабочая база не открывается), файл замеров —
  `<папка>\timings.tsv`. Без переменной поведение прежнее. Других переменных окружения программа не читает.
- `load_session` отдаёт `measure: true`, когда файл замеров задан. `Session.measure` хранит это значение.
  `App.svelte` после `await session.open(api)` при `session.measure` вызывает `installMeasure(api)`, а
  возвращённую функцию остановки — при размонтировании. Сигнатура `installMeasure(api)` признака `measure` не
  принимает: вызывать или нет, решает `App.svelte`.
- `installMeasure`:
  1. Через два кадра (`requestAnimationFrame` дважды) после вызова отправляет отдельным вызовом, не дожидаясь
     пачки, `[{ name: "ready", ms: performance.now() }]`. Так как вызов идёт после `session.open`, `ready` означает
     «окно отрисовано и вкладки восстановлены».
  2. Если `PerformanceObserver.supportedEntryTypes` содержит `"event"` — подписывается: `observe({ type: "event",
     durationThreshold: 16 })`. `16` — наименьший допустимый порог: наблюдатель отдаёт только записи с `duration`
     ≥ 16 мс, более быстрые события в файл не попадают вовсе.
  3. Записываются только: `keydown` и `input` (с любой меткой) и `click` с меткой `tab`. Остальное — `keyup`,
     `keypress`, `beforeinput`, события указателя и мыши, щелчки вне заголовка вкладки — не пишется.
  4. Метка — значение `data-measure` ближайшего предка цели записи (`entry.target.closest("[data-measure]")`) или
     `other` (в том числе когда цели нет). Метки: `tab` (заголовок вкладки, `TabBar`), `url` (`UrlBar`),
     `tree-filter` и `tree` (`Sidebar`), `body` (`BodyEditor`).
  5. На каждую запись — две строки (форма `TimingSample` не меняется): `<тип>:<метка>` с `ms = duration` (время от
     действия до следующей отрисовки, шаг 8 мс) и `<тип>:<метка>:proc` с `ms = processingEnd − startTime` (ожидание
     в очереди плюс обработка, без ожидания кадра).
  6. Раз в `MEASURE_FLUSH_MS` накопленное уходит одним вызовом `reportTimings`; пусто — вызова нет. Повтор —
     перезаводом `setTimeout`, не `setInterval` (поддельные таймеры тестов подменяют только `setTimeout`). Отказ
     `reportTimings` молчаливый.
- `scripts/measure-start.ps1` — по образцу `spike/shell/measure-start.ps1`. Параметры: `-Exe` (по умолчанию
  `target\release\requestmaster.exe` от корня репозитория), `-Dir` (обязателен; папка создаётся, если её нет),
  `-Runs 10`, `-TimeoutSec 30`, `-Keep`, `-Report`.
  - В начале — ошибка, если `requestmaster.exe` уже запущен (как в образце, строки 24–26).
  - Прогон (обычный режим, `-Runs` раз): удалить только `<Dir>\timings.tsv` (`app.db` и `bodies` не трогать — в
    базе лежат восстановленные вкладки); выставить процессу `RQM_MEASURE_DIR = <Dir>`; `Start-Process`; ждать
    появления строки `ready` в `timings.tsv` (опрос раз в 10 мс, отказ по истечении `-TimeoutSec`). В итог идёт время
    «по стене» от `Start-Process` до появления строки. Значение `ms` из самой строки (`performance.now()` окна)
    печатается справочно и в результаты не вносится. Закрытие — `CloseMainWindow()` с ожиданием выхода (как в
    образце: принудительное завершение только после 5 с), чтобы сессия успела записаться.
  - В конце — медиана и максимум времени «по стене».
  - `-Keep` — режим ручной работы: выставить `RQM_MEASURE_DIR`, запустить программу один раз, `ready` не ждать,
    программу не завершать, `timings.tsv` не удалять (строки дописываются).
  - `-Report` — программу не запускает: читает `<Dir>\timings.tsv` и печатает по каждому имени число строк, медиану
    и максимум (без этого критерий отклика из списка проверки по сотням строк руками не проверить).
  - Исполнителем скрипт не запускается (приложение запускать нельзя). Исполнитель проверяет его только разбором:
    `[System.Management.Automation.Language.Parser]::ParseFile(<путь>, [ref]$null, [ref]$errors)` — ошибок 0. В
    итогах пометить **[проверить]**.
- Тесты-замеры `core_perf.rs` помечены `#[ignore]`, печатают числа и ничего не утверждают о времени. Запускаются
  вручную и только в release-сборке (отладочная в разы медленнее, с целью спеки её сравнивать нельзя; обычный
  `cargo test --workspace` остаётся отладочным и эти тесты пропускает). `big_collection.rs` подключается так:
  `#[path = "common/big_collection.rs"] mod big_collection;`.
  - `request_overhead_report` — одна прогревочная отправка на wiremock (в выборку не входит, печатается отдельно:
    `first_request_overhead_ms=…`), затем 30 отправок; накладные расходы одной отправки — время вокруг
    `send_request` минус `total_ms` ответа. Печать: `request_overhead_ms median=… max=… n=30`.
  - `tree_of_3000_requests_report` — время `import_files` и `tree()` для `big_collection(30, 100)`. Печать:
    `import_3000_ms=…` и `tree_3000_ms=…`.
  - `write_big_collection_file` — пишет `target/measure/collection-3000.postman_collection.json` для владельца.

- [ ] **Step 1: Падающие тесты.** `core_data.rs`: `report_timings_without_a_measure_file_does_nothing`;
  `report_timings_appends_lines` (два вызова → четыре строки вида `ready\t812.5`); `load_session` отдаёт `measure`
  по конфигурации; `report_timings` в списке команд, число 37.
  `measure.test.ts` (`installMeasure` вызывается напрямую; `requestAnimationFrame` подменён на немедленный вызов;
  `PerformanceObserver` подменён через `vi.stubGlobal` классом со статическим `supportedEntryTypes = ["event"]`,
  который запоминает обработчик, — тест сам передаёт ему записи; цель записи — настоящий элемент внутри
  `[data-measure="…"]`): `ready` уходит один раз, отдельным вызовом; запись `{ name: "click", startTime: 200,
  processingEnd: 230, duration: 64 }` с целью внутри `[data-measure="tab"]` уходит через 2000 мс одним вызовом как
  `[{ name: "click:tab", ms: 64 }, { name: "click:tab:proc", ms: 30 }]`; запись `{ name: "keydown", startTime: 100,
  processingEnd: 107, duration: 24 }` с целью внутри `[data-measure="url"]` → `{ name: "keydown:url", ms: 24 }` и
  `{ name: "keydown:url:proc", ms: 7 }`; `keydown` с целью без метки → `keydown:other`; записи `keyup` в `url`,
  `click` вне `tab` и `pointerdown` не отправляются (через 2000 мс вызова нет); среда без типа `event`
  (`supportedEntryTypes = []`) — только `ready`; после функции остановки новые записи не отправляются.
  `App.test.ts`: при `measure: false` (сессия по умолчанию) после отрисовки окна и двух кадров `api.timingCalls`
  пуст — `reportTimings` не вызван ни разу; при `seed.session` с `measure: true` — `waitFor` находит вызов с
  `ready`.
- [ ] **Step 2: Запустить** `cargo test -p requestmaster --test core_data` и `npm run test --
  src/lib/measure.test.ts src/App.test.ts` — новые случаи падают.
- [ ] **Step 3: Реализовать**; команда в четырёх местах; во все литералы `CoreConfig` дописать `measure_file: None`.
  Литералы `SessionDto` в тестах Task 10 (`Session.test.ts`, `tabs.test.ts`, `App.test.ts`) не правятся: тип
  `FakeSeed.session` делает `measure` необязательным, `createFakeApi` подставляет `false`.
- [ ] **Step 4: `cargo test --workspace`, clippy, `npm run check`, `npm run test`, `npm run build`** — зелёные.
  Затем замер: `cargo test --release -p requestmaster --test core_perf -- --ignored --nocapture --test-threads=1`
  выполняется и печатает числа в названном формате. Первая release-сборка долгая (`lto = true`, `codegen-units =
  1`) — это ожидаемо, не зависание. Cargo при этом пересобирает и `target\release\requestmaster.exe`, поэтому
  перед запуском убедиться, что программа не запущена (`tasklist`); если запущена — замер пропустить и отметить в
  отчёте (он повторяется в Task 18 Step 2). Разбор `scripts/measure-start.ps1` — ошибок 0.
- [ ] **Step 5: Commit** — «План 2: режим замера и замеры раздела 10».

---

### Task 18: Сборка, замеры, список проверки для владельца, документы

**Files:**
- Create: `stage1-part3b-results.md`
- Create: `stage1-part3b-owner-checklist.md`
- Modify: `docs/design.md`

- [ ] **Step 1: Полный прогон:** `cargo test --workspace`, clippy; в `ui/` — `npm run check`, `npm run test`, `npm
  run build`; `npm audit` (сравнить с двумя прежними замечаниями низкой важности). `cargo tree -p <crate> -i tauri`
  для каждого `rqm-*` — пакета нет; `git ls-files spike/samples` — пусто. Поиск мёртвого кода: `rqm.layout` и
  `layout.load` по `ui/src` — пусто; `localStorage` по `ui/src` вне `*.test.ts` — пусто, а в тестах слово есть
  только в `layout.test.ts`, в одной проверке, что `choose` и `resize` в него не пишут (как записано в Task 10);
  `save_as` по `src-tauri` — пусто.
- [ ] **Step 2: Замеры без окна** (в release-сборке; до Step 3): убедиться, что программа не запущена (`tasklist`
  не показывает `requestmaster.exe` — cargo пересобирает и exe пакета; если запущена — вернуть `BLOCKED`, как в
  Step 3). `cargo test --release -p requestmaster --test core_perf -- --ignored --nocapture --test-threads=1` —
  записать из строки `request_overhead_ms median=… max=… n=30` медиану и максимум накладных расходов (цель ≤ 20
  мс), отдельно `first_request_overhead_ms`, а также `import_3000_ms` и `tree_3000_ms`; в итогах явно написать
  «снято в release, без окна». Первая release-сборка долгая (`lto`) — не считать зависанием. Файл
  `target/measure/collection-3000.postman_collection.json` создан. После Step 3 этот замер не повторять: он
  заменяет `target\release\requestmaster.exe` сборкой cargo, а не `tauri build` (другой набор возможностей Tauri),
  а замеры владельца должны идти на exe из Step 3 (если повторить пришлось — повторить и Step 3).
- [ ] **Step 3: Собрать приложение:** убедиться, что программа не запущена (`tasklist` не показывает
  `requestmaster.exe`; если запущена — не собирать, вернуть `BLOCKED` с просьбой к владельцу закрыть её); в `ui/` —
  `npm run tauri build`. Приложение не запускать. Записать размеры exe и установщика (цель — установщик ≤ 20 МБ) и
  размеры JavaScript и CSS сборки.
- [ ] **Step 4: Список проверки для владельца** — на русском, простыми словами, пронумерованные шаги с «что должно
  получиться», по разделам:
  1. *Обновление:* первый запуск новой сборки — данные и история на месте; расположение ответа нужно выбрать заново
     (один раз).
  2. *Строгая CSP (первым делом):* редактор тела, ответа и скриптов выглядит как раньше — подсветка, номера строк,
     выделение, сворачивание; меню и подсказки на месте; боковая панель меняет ширину. Если нет — строки отката из
     Task 16.
  3. *Дерево:* перетащить запрос в другую папку и в другую коллекцию (запрос берёт auth и переменные нового места);
     переставить папки и коллекции; при перетаскивании узла над допустимой строкой курсор не показывает запрет и
     бросок срабатывает (в автотестах этого не видно — проверяется только здесь); попытка бросить папку в саму
     себя — курсор запрета, ничего не меняется; бросок в свёрнутую папку; прокрутка у края при перетаскивании;
     файл с диска, перетащенный в окно, по-прежнему импортируется и не открывается в окне вместо программы;
     Alt+↑ ↓ ← →; под фильтром перемещения нет;
     меню папки «Открыть» — Auth, Scripts, описание; коллекция — Variables, Auth, Scripts, описание; экспорт в
     Postman после правки описания.
  4. *Большое дерево* (выполняется внутри шага «б» раздела 12 — программа запущена в режиме замера, рабочая база
     не затронута): импортировать `target\measure\collection-3000.postman_collection.json`, развернуть всё,
     прокрутка и фильтр без задержек; стрелки и F2 на строке у нижнего края; фокус не теряется; после прокрутки
     мышью Tab попадает в дерево.
  5. *Переменные:* цвета областей в URL, Params, Headers, теле; подсказка по наведению (значение и область; секрет —
     точки); ненайденная — красное подчёркивание и число у Send; `{{` и список, Enter, Escape; смена окружения
     меняет цвета; в теле JSON с `{{id}}` без кавычек цвет переменной виден.
  6. *Память окна:* открыть несколько вкладок, внести правки без сохранения, сдвинуть разделители, закрыть и
     открыть программу — вкладки, активная вкладка, точки «не сохранено», текст правок, ширина панели и
     расположение ответа на месте; ответа во вкладках нет; удалить запрос с открытой вкладкой, перезапустить —
     ошибок нет.
  7. *Настройки:* таймаут 1000 мс и медленный адрес — «Сервер не ответил вовремя»; «Без прокси» / «Вручную» с
     ошибкой в адресе; выключить проверку SSL и отправить запрос на адрес с самоподписанным сертификатом; настройка
     запроса «Проверять SSL: Да» переопределяет общую.
  8. *Scripts и Settings:* текст скриптов с подсветкой сохраняется и уходит в экспорт; полоса «Скрипты не
     выполнены»; «Следовать редиректам: Нет» — ответ 3xx как есть; «Максимум редиректов»; тело у GET.
  9. *Тело:* form-data с текстом и двумя файлами; импортированный запрос с файлом — пометка и выбор заново; binary;
     окно выбора файла поверх главного окна, главное недоступно, пока оно открыто.
  10. *Ответ:* «Сохранить в файл» для обычного ответа и для ответа больше 20 МБ (файл целый); отмена в окне
      сохранения; Ctrl+F в ответе, в теле запроса, вне редактора; панель поиска на русском; строки поиска браузера
      нет.
  11. *Полоса вкладок и окружение без имени:* колесо мыши; активная вкладка видна после открытия из дерева; рамки
      после щелчка нет, после Tab и стрелок есть; окружение «Без названия» правится и сохраняется.
  12. *Замеры (раздел 10 спеки)* — по порядку; программа перед каждым шагом закрыта; `X` — одна и та же новая
      папка для шагов «б»–«г» (рабочая база не открывается):
      а) Холодный запуск: `scripts\measure-start.ps1 -Dir <новая пустая папка> -Runs 10` → медиана и максимум
         времени «по стене», цель ≤ 1 с.
      б) Подготовка и работа: `scripts\measure-start.ps1 -Dir X -Keep` → программа открыта в режиме замера.
         Импортировать `target\measure\collection-3000.postman_collection.json`, открыть 10 вкладок запросов,
         выполнить раздел 4, затем работу на отклик: 20 переключений вкладок щелчком; по 50 нажатий клавиш в поле
         URL, в теле запроса и в фильтре дерева на 3000. Подождать не меньше 2 с (замеры уходят в файл пачкой раз
         в 2 с) и закрыть программу обычным способом.
      в) Сразу после закрытия, до шага «г» (он удалит файл): `scripts\measure-start.ps1 -Dir X -Report` печатает
         по каждому имени число строк, медиану и максимум. Ввод (`keydown:url`, `keydown:body`,
         `keydown:tree-filter` и строки `input:…`): медиана строк с окончанием `:proc` ≤ 16 мс и ни одной строки
         без `:proc` больше 32 мс — отклик попал в ближайший кадр, пропущенного кадра нет; допустим единичный
         выброс на первом нажатии. Переключение вкладки (`click:tab`, без `:proc`): медиана ≤ 50 мс, максимум
         записать. Пояснение: значения 16 и 24 в строках без `:proc` — это ожидание кадра на мониторе 60 Гц, а не
         провал цели; события быстрее 16 мс в файл не попадают вовсе, поэтому отсутствие строк у имени значит
         «всё быстрее 16 мс».
      г) Запуск с вкладками: `scripts\measure-start.ps1 -Dir X -Runs 10` → медиана и максимум (десять
         восстановленных вкладок и дерево на 3000).
      д) Числа шагов «а», «в», «г» вписать в таблицу документа результатов (строки «ожидает замера владельца») и
         в колонку «Этап 1» раздела 10 спеки.
- [ ] **Step 5: Документ результатов** — что сделано; числа тестов и сборки; замеры Step 2 (с пометкой «снято в
  release, без окна») и строки «ожидает замера владельца»; как меряется цель «отклик на ввод ≤ 16 мс» (время
  обработки `:proc` плюс условие «без пропущенного кадра», решение № 38); все решения из раздела «Решения автора»
  с тем, как они реализованы; решения исполнителей там, где план молчал; отклонения от плана; новые зависимости;
  ключи `app_state`; чем закрыт Review Focus; закрытые отложенные замечания (W-b, W-l, W-o, T-n для закрытия окна,
  T-s, T-t) и оставшиеся (в том числе T-n для закрытия вкладки окружения); что не входит; пометки **[проверить]**
  (скрипт замера, CSP в живом окне, перетаскивание в живом окне).
- [ ] **Step 6: Обновить спеку:** раздел 4 — новые команды и ссылка на «Контракт команд» этого плана, упоминание
  `format_json`; раздел 5 — таблица ключей `app_state` (пять ключей) и фраза «версия схемы 2 не менялась»; раздел 7
  — единицы и границы таймаута, проверка адреса прокси; раздел 9 — папка открывается из меню, перемещение Alt+стрелки,
  правило счётчика у Send, вопрос при закрытии окна только при сбое записи, расположение ответа хранится в базе;
  раздел 10 — колонка «Этап 1» с измеренным и уточнение метода: «отклик на ввод ≤ 16 мс» — медиана времени
  обработки события ≤ 16 мс и ни одного пропущенного кадра (решение № 38); раздел 11 — CSP; раздел 12 — план 2 выполнен, нераспределённые
  требования из раздела «Не входит в план».
- [ ] **Step 7: Commit** — «План 2: сборка, замеры, список проверки и итоги».
