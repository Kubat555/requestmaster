# RequestMaster, этап 2 «Скрипты» — план реализации

**Goal:** Скрипты Postman (pre-request и test коллекции, папок и запроса) исполняются при Send в том же порядке и с
тем же потоком данных, что в Postman: меняют переменные, запрос, проверяют ответ через `pm.test` / `pm.expect`,
шлют `pm.sendRequest`; окно показывает результаты тестов, консоль и ошибки со строкой; скрипты импортированной
коллекции исполняются только после согласия.

**Architecture:** Новый crate `crates/rqm-script` встраивает QuickJS (`rquickjs`) и исполняет скрипты в отдельном
потоке ОС под пределами времени, памяти и стека; весь `pm.*` — JavaScript-прелюдия, которая зовёт узкий набор
примитивов хоста на Rust (каждый предел проверяет Rust, а не прелюдия). Скрипт, который движок не может прервать,
через запас времени бросается сторожем: отправка получает ошибку, а поток доживает сам. `rqm-runner` превращает `send` в конвейер «pre-request → подстановка → HTTP → test» и
возвращает отчёт скриптов вместе с ответом; `pm.sendRequest` идёт через тот же `HttpEngine`. Оболочка `src-tauri`
после отправки записывает изменения переменных дельтой поверх свежего чтения базы и отдаёт окну тесты, консоль,
ошибки и ту же дельту; окно применяет её к открытым редакторам. Событий «ядро → окно» нет: всё приходит ответом
команды `send_request`.

**Tech Stack:** то же, что на этапе 1. Новое **[моё решение]**: crate `rquickjs = "=0.14.0"` (QuickJS-NG 0.16.2,
MIT, только feature по умолчанию `std`; в `Cargo.lock` добавляются `rquickjs`, `rquickjs-core`, `rquickjs-sys`,
`allocator-api2` — они входят в сборку, — и `relative-path` 2.0.3 (MIT OR Apache-2.0): это только запись lock из-за
слабой зависимости feature `std = ["relative-path?/std"]`, в сборку пакет не входит и `cargo tree` его не
показывает); npm-пакеты сборки прелюдии в `crates/rqm-script/prelude/package.json` — `chai 4.4.1` (MIT, та же
версия, что в `postman-sandbox` 6.9.0), `lodash 4.18.1` (MIT, версия Postman), `esbuild 0.28.2` (только инструмент
сборки). В окне новых зависимостей нет (`@codemirror/autocomplete 6.20.4` уже прямая; `@types/node` не
добавляется — тесты окна читают файлы только импортом `?raw`).

**Spec:** `docs/design.md` (разделы 3, 4, 5, 6, 8, 9, 10, 11, 12, 13).
Формат и соглашения — предыдущий план `docs/plans/stage1-part3b-tree-variables-settings.md`.
Образец приёмов движка — `spike/script-probe/` (не продукт).
Два утверждения `postman-api.md` с пометкой [память] этим планом опровергнуты по исходникам `postman-runtime`
7.56.3 и при расхождении прав план: в test-скрипте `pm.request` — **отправленный** запрос (раздел 3.3 записок
говорит обратное); `pm.sendRequest` `{{переменные}}` **не** подставляет (строка записок о подстановке неверна).

## Решения владельца (2026-11-18) — важнее текста задач

Владелец согласовал план словами «делай по плану, скрипты активируй и разреши сразу, тесты делай только для
важных частей, излишние не нужны». Где текст задачи ниже расходится с этим разделом — прав этот раздел;
исполнитель записывает расхождение в `plan_deviations`.

1. **Скрипты работают сразу, без нажатия «Разрешить».** Механизм согласия (`scripts_allowed`, команды, галочка
   в редакторе коллекции, предупреждение `scripts_not_allowed`) остаётся как способ **запретить** скрипты
   отдельной коллекции, но по умолчанию всё разрешено:
   - импорт коллекции добавляет её id в `scripts_allowed` (как `create_node`); копия любой коллекции разрешена,
     если разрешён оригинал;
   - если строки `scripts_allowed` в базе нет (база этапа 1), при открытии хранилища ядром (`AppCore`) она
     создаётся со списком id **всех** существующих коллекций — один раз, дальше строка есть. Это единственный
     новый DML сверх раздела «Изменения в базе данных»; схема не меняется;
   - блок согласия в отчёте импорта (Task 15) не нужен: вместо кнопки «Разрешить скрипты» — ничего; галочка
     «Разрешить скрипты этой коллекции» в `CollectionEditor` и кнопка «Разрешить» в предупреждении остаются;
   - тесты плана, которые ждут «импортированная коллекция не разрешена», меняются на: импортированная
     разрешена; запрет проверяется после `set_scripts_allowed(id, false)`.
2. **Общий выключатель `scripts_enabled`** — делается, по умолчанию включён (как в плане).
3. **`pm.execution.skipRequest`** в первой версии — остаётся (решение № 36). Пределы и сторож (№ 37, 38),
   № 40, № 41 — приняты.
4. **Ветка** — `feature/stage2-scripts`.
5. **Тесты — только для важного.** Обязательны тесты, которые охраняют: пределы и безопасность движка (время,
   память, стек, сторож, утечка примитивов), поток и запись переменных, порядок конвейера, `pm.sendRequest`,
   запись в историю, контракт команд, набор совместимости Task 17, пункты Review Focus. По одному тесту на
   поведение — без таблиц вариантов одного и того же. **Не писать:** тесты разметки и текстов компонентов окна,
   тесты анимаций и оформления, повторы уже покрытого на другом уровне. В задачах окна (13–16) — только тесты
   логики (применение дельты переменных к открытым редакторам, разбор результата отправки, источник
   автодополнения). Пропущенные тесты плана исполнитель перечисляет в отчёте одной строкой каждый с причиной;
   ревьюер не считает их дефектом, если важное поведение покрыто.

## Global Constraints

- Ветка — та, в которой владелец запустил исполнение (сейчас `feature/stage1-part3`; см. «Вопросы владельцу», № 4).
  В `main`/`master` не коммитить. Push — никогда. Шаги «Commit» выполняются по команде владельца.
- Сообщения коммитов на русском.
- В рабочем дереве на момент написания плана есть незакоммиченные правки владельца в
  `ui/src/components/ResponsePanel.svelte` и `ui/src/styles/tokens.css`. Их не откатывать и не «чинить»; задачи,
  которые правят эти файлы (Task 14, Task 15), работают поверх того, что в них лежит.
- После последней задачи, до завершения ветки, — итоговое код-ревью (правило владельца); его отчёт прикладывается
  к итогам.
- Приложение, собранный exe и dev-серверы не запускать; `npm run tauri build` — только в Task 19 и только когда
  `requestmaster.exe` не запущен. `--release` — только там, где задача прямо это говорит (Task 18 Step 4, Task 19).
  Разрешено: `cargo build/test/clippy`; в `ui/` и в `crates/rqm-script/prelude/` — `npm install`, `npm run …`.
- Сеть: crates.io и npm. Тесты ходят только на `127.0.0.1`; в тестах отправки — `NO_PROXY=127.0.0.1`.
- Версии зависимостей закреплять точно (`=x.y.z` в корневом `Cargo.toml`, точная версия в `package.json`). В дереве
  нет aws-lc / native-tls / openssl. У `rquickjs` не включать features `parallel`, `futures`, `loader`, `macro`,
  `bindgen`, `disable-assertions`, `rust-alloc`.
- **База данных:** DDL нет, `PRAGMA user_version` остаётся `2`, `schema.sql` и `schema_v2.sql` не трогать (в том
  числе ради комментариев). Разрешено только перечисленное в разделе «Изменения в базе данных». Любая таблица,
  колонка, индекс или ключ сверх этого — остановиться и спросить.
- **Сохранённый JSON читается всегда.** Каждое новое поле структур, которые лежат в базе (`SettingsDto`,
  `HistoryDetail`), — с `#[serde(default…)]`. Варианты перечислений, уже записанных в базу (`WarningKind`,
  `SendErrorKind`), не удаляются и не переименовываются — только добавляются.
- Crates ядра не зависят от Tauri. `rqm-script` зависит только от `rquickjs`, `rqm-vars`, `serde`, `serde_json`,
  `tokio-util` — не от `rqm-http`, `rqm-store`, `rqm-model`, `rqm-runner`. Направление: `src-tauri` → `rqm-runner` →
  `rqm-script`.
- **Правила движка** (каждое — подтверждённая опытом ловушка, `engine.md` разделы 4.7, 7):
  1. `Runtime` и `Context` создаются и уничтожаются внутри потока скрипта; поток создаёт сам `rqm-script` со стеком
     `THREAD_STACK_BYTES` (16 МиБ); в `set_max_stack_size` никогда не передаётся больше `MAX_STACK_BYTES` (8 МиБ —
     половина стека потока). Две ловушки: значение больше 16 МиБ **отключает** проверку стека в rquickjs; значение,
     равное стеку потока, проверку не отключает, но она не успевает сработать раньше настоящего переполнения, и
     процесс умирает (проверено опытом: поток 16 МиБ и предел 8, 12 и 15 МиБ дают `RangeError`, предел 16 МиБ —
     `thread has overflowed its stack`).
  2. Rust не хранит JS-значения (`Function`, `Value`, `Object`, `Promise`) дольше одного вызова `Context::with`:
     таблицы callback'ов (таймеры, `pm.sendRequest`, `done`) живут в прелюдии, Rust знает только номера. Иначе
     QuickJS завершает процесс по assert при уничтожении рантайма.
  3. В `rqm-script` нет паник: в `src/lib.rs` стоит
     `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing,
     clippy::string_slice, clippy::unreachable, clippy::todo, clippy::unimplemented))]`. Деление и остаток — только
     `checked_div` / `checked_rem`; счётчики и суммы пределов — `saturating_*` / `checked_*`
     (`clippy::arithmetic_side_effects` целиком не включается: он срабатывает на любое `+`). В release профиль
     `panic = "abort"` — паника в потоке скрипта закрывает приложение со всеми несохранёнными правками, и ветка
     «поток завершился паникой → ошибка `Internal`» там **недостижима**: она работает только в dev- и
     test-профиле. Единственная защита в продукте — правила этого пункта и пунктов 8, 9.
  4. Скрипт пользователя исполняется нестрого (`EvalOptions.strict = false`), с именем файла `script.js`, в обёртке
     `(async()=>{` + текст + `\n})()` — номера строк совпадают с редактором.
  5. Причину остановки (таймаут, отмена, предел памяти хоста, брошен сторожем) определяют собственные флаги, а не
     текст исключения.
  6. **Пределы считает Rust.** Скрипт и прелюдия живут в одном realm: скрипт может подменить любой встроенный
     объект (`Function.prototype.call / apply / bind`, `Array.prototype.*`, `String.prototype.slice`,
     `JSON.stringify`, `Promise.prototype.then`) раньше, чем им воспользуется прелюдия, и так добраться до
     примитива хоста. Поэтому каждый предел и каждая проверка выполняются в Rust-примитиве (`src/host.rs`,
     `src/variables.rs`), прелюдия — только удобство: длина строки консоли, число строк и байт консоли; длина имени
     и текста ошибки теста, число тестов и суммарный объём их текста; длина значения переменной, объём переменных
     и журнала изменений; число `pm.sendRequest`, размер запроса и очереди; фаза (примитивы изменения запроса и
     `skip_request` в `Phase::Test` бросают исключение JS, а `Report.request` и `Report.skip_request` фазы test
     вызывающий не читает); номера таймеров и callback'ов, пришедшие из JS, Rust проверяет сам. Отсутствие файлов
     обеспечено типами: у `OutgoingBody` и `ScriptBody` нет варианта с путём — и не появляется.
  7. Второй слой (не основа безопасности): модули прелюдии зовут примитивы только прямым вызовом (`host.log(x)`) —
     не через `.call / .apply / .bind`, не передают их как callback и не кладут в объекты, видимые скрипту; нужные
     встроенные функции (`toWellFormed`, `JSON.stringify`, `Reflect.apply`) сохраняются в замыкание при загрузке
     прелюдии. Каждая функция-примитив получает имя с приставкой `rqm_host_` (`Function::with_name`) — по нему
     тест `host_primitives_do_not_leak` узнаёт утечку.
  8. **Текст на границе.** (а) Текст режется только по границе символа — один общий помощник `clip(text,
     max_chars)` в `src/text.rs` (по образцу `text_prefix` в `src-tauri/src/history.rs`); единица всех пределов
     «в символах» — скалярное значение Unicode (`chars()`), а не единица UTF-16 и не байт. (б) Строка JS с
     одиночным суррогатом не превращается в `String` Rust (`rquickjs::String::to_string` возвращает ошибку —
     проверено опытом), поэтому прелюдия пропускает каждую строку, уходящую в примитив, через сохранённый
     `String.prototype.toWellFormed` (одиночный суррогат → U+FFFD); строки, которые Rust читает сам (текст
     исключения), он пропускает через ту же встроенную функцию, взятую из контекста **до** исполнения текста
     скрипта. (в) `rquickjs::CString::as_str` не использовать: внутри `from_utf8_unchecked`. Длина строки до
     копирования в Rust берётся как `String::to_cstring()?.len()` (копия лежит в куче QuickJS, под её пределом),
     сам текст — `String::to_string()`. (г) Приведения типов на границе нет (число вместо строки — ошибка
     преобразования), прелюдия сама делает `String(x)`.
  9. **Состояние хоста** (консоль, тесты, переменные, журнал, очередь запросов) лежит в `Arc<Mutex<Option<…>>>`,
     общем для потока скрипта и для `run`. Примитив берёт замок через `try_lock` и не держит его через вызов в JS
     (`bundle::load`, вызов callback'а таймера, функция учёта отклонённых Promise, `eval`); отказ `try_lock` или
     `None` внутри — исключение JS, не паника и не ожидание. Отравленный замок читается
     (`PoisonError::into_inner`).
  10. **Сторож.** Обработчик прерывания QuickJS не вызывается внутри части нативных циклов движка
      (`new Array(2**32-1).join('')`, `[].reverse.call({length: 2**53-1})`, `.sort()`, `.splice`, `.shift`,
      `.copyWithin`, `.flat`, `[].concat` — проверено опытом: не остановились за 20 с при пределе 0,5 с), поэтому
      ни таймаут, ни отмена такой скрипт не остановят. `run` ждёт поток не дольше «предел времени + запас»
      (Task 1) и бросает его: отчёт забирается, поздние обращения брошенного потока к хосту не исполняются.
- Прелюдию собирает только `npm run build` в `crates/rqm-script/prelude/`; собранные файлы лежат в
  `crates/rqm-script/prelude/built/` и коммитятся (каталог `dist/` в `.gitignore` — им не пользоваться). `cargo
  build` и `cargo test` Node не вызывают. `npm install` запускать изнутри каталога с `package.json`.
- В TypeScript нет исполнения скриптов, разбора Postman-JSON, подстановки переменных, сборки HTTP-запроса.
  Применение готовой дельты переменных к строкам открытого редактора (Task 13) и статический список подсказок
  (Task 16) — не логика скриптов.
- Вывод скрипта (консоль, имена и сообщения тестов, текст ошибок) — недоверенный текст: в разметке только
  `{text}`, никакого `{@html}`. В разметке Svelte нет статических атрибутов `style="…"` — только директивы `style:`.
- Тексты интерфейса — в `ui/src/lib/strings.ts` под английскими ключами, на русском; термины HTTP и Postman — на
  английском. В компонентах и в Rust русского текста нет. Цвета и размеры — только через `ui/src/styles/tokens.css`;
  новая пара «цвет/фон» добавляется в `ui/src/styles/contrast.test.ts`.
- Каждая команда окна — в четырёх местах: `COMMANDS` (`src-tauri/src/lib.rs`), `src-tauri/build.rs`,
  `generate_handler!` (`src-tauri/src/main.rs`), `capabilities/default.json` (`allow-<имя-через-дефис>`); плюс список
  и число в тесте `commands_are_listed_everywhere` (`src-tauri/tests/core_data.rs`). Сейчас команд 37; после
  Task 11 — 39. Каждый новый метод `Api` появляется сразу в `tauriApi` и в `lib/testing/fakeApi.ts`.
- Команды — `#[tauri::command(rename_all = "snake_case")] pub async fn … -> Result<_, String>`; ключи аргументов и
  ответов — `snake_case`; замок хранилища через `.await` не удерживается. Верхний `postman_extra` узла в окно не
  передаётся. Правила P1–P4 спеки (раздел 5): переменная, изменённая скриптом, — «изменённая строка списка» по P3,
  её `postman_extra`, `secret`, `enabled` не трогаются.
- Данные владельца: `spike/samples/` не открывать, не печатать, имена файлов не упоминать; тесты на них печатают
  только числа. В git не попадает.
- Только то, что в задаче. Доступность: всё с клавиатуры, у кнопок-значков `aria-label`, фокус виден.
- Поддельные таймеры в тестах окна — как в плане 2: `vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] })`
  после `findBy*`, в `afterEach` — `vi.useRealTimers()`.
- Rust-задача заканчивается зелёными `cargo test --workspace` и `cargo clippy --workspace --all-targets -- -D
  warnings`; UI-задача — зелёными `npm run check`, `npm run test`, `npm run build` в `ui/`. Существующие проверки не
  ослабляются; изменённые утверждения перечисляются в отчёте задачи с причиной.
- Мёртвый код: задача, которая что-то заменяет, удаляет старое в том же коммите после поиска по всему репозиторию
  и называет удалённое в отчёте.

## Review Focus

1. **Скрипт, который не останавливается или ломает движок.** `while(true){}`, «катастрофический» regex,
   бесконечная рекурсия, рост памяти **движка и хоста** (`while (true) pm.globals.set('k', 'x'.repeat(1 << 20))`),
   нативный цикл, который движок не прерывает (`[].reverse.call({length: 2**53-1})`), `setInterval` без
   остановки, `while(true) console.log(1)`, Promise, который никто не разрешит, Cancel посреди скрипта и посреди
   `pm.sendRequest`, строки с одиночным суррогатом и не-ASCII на границах обрезки — процесс жив, отправка
   заканчивается ошибкой скрипта понятного вида за ограниченное время, следующая отправка работает. Ограниченное
   время обеспечивает **сторож** (`run` перестаёт ждать поток), а не один обработчик прерывания; память хоста —
   пределы в примитивах Rust (тесты в Task 1 — `watchdog.rs`, `watchdog_cap.rs`; Task 2 —
   `endless_set_is_a_memory_error`, `tampered_builtins_do_not_lift_limits`; Task 3, 6, 7, 10, 12).
2. **Скрипт пишет переменную, а её редактор открыт.** Вкладка окружения с несохранённой правкой, вкладка
   коллекции с черновиком, отправка отменена или вытеснена следующей — значение скрипта доходит до базы и до
   редактора, чужие правки не затираются, Ctrl+S коллекции не возвращает старое значение (тесты в Task 10, 11, 13).
3. **Данные сборки этапа 1.** Настройки без новых полей, записи истории без тестов и с предупреждением
   `scripts_not_executed`, коллекции, импортированные до появления согласия, — всё читается; таймаут, прокси и
   проверка сертификата не сбрасываются; история открывается (тесты в Task 11, 12).
4. **Скрипт коллекции, которой не доверяют.** До согласия не исполняется ни одна строка; после согласия скрипт не
   получает файлов: `pm.sendRequest` с телом-файлом, превращение тела запроса в файл из pre-request, `fetch`,
   `process`, `require('fs')` — недоступны; объект `__host` из скрипта не виден, а прямой вызов примитива (если
   скрипт добрался до него подменой встроенных объектов) не даёт ничего сверх `pm.*` — пределы и запреты
   проверяет Rust (тесты в Task 1, 2, 4, 6, 8, 10).
5. **Тело ответа, которое скрипту не по силам.** 50 МБ, не-UTF-8, с BOM, пустое, не-JSON — `pm.response.json()`
   и `responseBody` либо работают, либо дают понятную ошибку теста; память хоста не растёт сверх предела, ответ
   пользователю показан в любом случае (тесты в Task 3, 4).

## API в первой версии

Колонка «У владельца» — число вхождений в скриптах единственного образца владельца (счётчик печатает только имена
API и числа; `postman-api.md`, раздел 2). «Позже» значит: вызов бросает понятную ошибку `<имя> is not supported in
RequestMaster` (а не `undefined is not a function`), а отчёт импорта называет его заранее (Task 9, Task 12).

| API | Первая версия | У владельца | Задача |
|---|---|---|---|
| `pm.collectionVariables.set` | да | **3** | 2 |
| `pm.environment`, `pm.globals`, `pm.collectionVariables`, `pm.variables`: `get`, `set`, `unset`, `has`, `clear`, `toObject`, `replaceIn` | да; третий аргумент `set` (тип) принимается и не используется | 0 (кроме строки выше) | 2 |
| `pm.environment.name` | да | 0 | 2 |
| `.toJSON()`, `.values` областей | позже | 0 | — |
| `pm.info`: `eventName`, `requestName`, `requestId`, `iteration` (всегда `0`), `iterationCount` (всегда `1`) | да; `requestId` — id узла RequestMaster | 0 | 2 |
| `console.log / info / warn / error / debug` | да | 0 | 2 |
| `console.clear` | да, ничего не делает | 0 | 2 |
| `console.table / trace / assert` | нет (как в Postman: `TypeError`) | 0 | — |
| `pm.response`: `code`, `status`, `reason()`, `headers` (список), `responseTime`, `responseSize`, `text()`, `json()` | да | 0 | 3 |
| `pm.response.cookies`, `size()`, `jsonp()`, `dataURI()`, `mime()`, `contentInfo()`, `encoding()`, `originalRequest`, `timingPhases()` | позже | 0 | — |
| `pm.test(name, fn)`, `pm.test.skip`, `pm.test.index()` | да | 0 | 3 |
| `pm.test` с `done` и с функцией, вернувшей Promise | да (Promise ожидается — надмножество Postman) | 0 | 5 |
| `pm.expect` | да — chai 4.4.1 целиком | 0 | 5 |
| `pm.response.to.have.status / header / body / jsonBody / statusCode / statusReason / statusCodeClass`; `pm.response.to.be.ok / info / success / redirection / clientError / serverError / error / accepted / withoutContent / badRequest / unauthorized / unauthorised / forbidden / notFound / notAcceptable / rateLimited / json / withBody`; отрицание `to.not…` | да | 0 | 5 |
| `pm.response.to.have.jsonSchema`, `to.have.responseTime`, `to.have.responseSize`, `pm.request.to.*` | позже | 0 | — |
| `setTimeout`, `clearTimeout`, `setInterval`, `clearInterval`, `setImmediate`, `clearImmediate`; `async/await`, `await` на верхнем уровне | да | 0 | 6 |
| `pm.sendRequest(req, callback)` и `pm.sendRequest(req)` → Promise; `req` строкой или объектом; тело `raw`, `urlencoded`, `formdata` (только текст) | да; третий аргумент callback (`history`) не передаётся; `{{переменные}}` в запросе скрипта **не подставляются**, как в Postman (нужна подстановка — `pm.variables.replaceIn`) | 0 | 6, 7 |
| `pm.request`: `method`; `url` (`toString()`, `getRaw()`, `getHost()`, `getPath()`, `getQueryString()`, `getPathWithQuery()`, присваивание строки, `query`: `get/has/add/upsert/remove/all/each/toObject/count`, `addQueryParams`, `removeQueryParams`); `headers`: `get/has/add/upsert/remove/all/each/toObject/count`; `addHeader`, `removeHeader`, `upsertHeader`; `body`: `mode`, `raw`, `urlencoded`, `toString()`, `isEmpty()`, `update()` | да; в pre-request — запрос как введён (`{{…}}` не подставлены), изменения уходят в запрос; в test — **отправленный** запрос: переменные подставлены, заголовок авторизации есть, изменения ни на что не влияют. Отличие от Postman: автоматические заголовки (`User-Agent`, `Host`, `Content-Length`, `Accept-Encoding` …) в test не видны | 0 | 8 |
| `pm.request.url.variables / protocol / host / port / path / hash`, `pm.request.auth`, изменение `formdata` / `file` / `graphql`, `pm.request.id / name / size() / toJSON() / clone()` | позже | 0 | — |
| `pm.execution.skipRequest()` | да (только pre-request, как в Postman) — **отступление от спеки §8**, решение № 36 | 0 | 8 |
| `pm.execution.setNextRequest`, `postman.setNextRequest` | да, ничего не делает (нужен только Runner) | 0 | 8 |
| `pm.execution.location`, `pm.execution.runRequest` | позже | 0 | — |
| `responseBody` | да | **3** | 3 |
| `responseCode` (`{ code, name, detail }`), `responseHeaders`, `responseTime` | да | 0 | 3 |
| `tests["имя"] = значение` | да | 0 | 5 |
| `postman.setEnvironmentVariable / getEnvironmentVariable / clearEnvironmentVariable / clearEnvironmentVariables`, `postman.setGlobalVariable / getGlobalVariable / clearGlobalVariable / clearGlobalVariables`, `postman.getResponseHeader` | да | 0 | 8 |
| `environment`, `globals` (объекты-снимки; `postman.set* / clear*` меняют и их), `request` (старый: `id`, `name`, `method`, `url`, `headers` — ключи в нижнем регистре, `data`), `data` (`{}`), `iteration` (`0`) | да | 0 | 8 |
| `postman.getResponseCookie`, `responseCookies`, `xml2Json` | позже | 0 | — |
| `require('lodash')` (4.18.1), `require('chai')`, `require('atob')`, `require('btoa')`; глобальные `atob`, `btoa`; глобальный `_` — lodash **4** (в Postman — lodash 3) | да | 0 | 9 |
| `require` для `crypto-js`, `moment`, `ajv`, `tv4`, `uuid`, `xml2js`, `cheerio`, `csv-parse/lib/sync`, `postman-collection`, `backbone`, `path`, `assert`, `buffer`, `util`, `url`, `punycode`, `querystring`, `string_decoder`, `stream`, `timers`, `events`; глобальные `CryptoJS`, `tv4`, `cheerio`, `Backbone`, `Buffer` | позже (спека §8: поднимаются, когда встретятся у владельца — не встретились) | 0 | — |
| `pm.iterationData` (`get` → `undefined`, `has` → `false`, `toObject` → `{}`), `pm.visualizer.set / clear` | да, пустые (спека относит оба к «позже» — решение № 36) | 0 | 9 |
| `pm.cookies`, `pm.cookies.jar()`, `pm.vault`, `pm.require`, `pm.datasets`, `pm.message` | позже | 0 | — |
| `JSON.parse` и остальной язык: ES2023 целиком, `Promise`, `Map/Set`, `Proxy`, регулярные выражения с именованными группами | да (сам движок) | **3** (`JSON.parse`) | 1 |
| `Intl` и форматирование по локали, `URL`, `URLSearchParams`, `TextEncoder`, `TextDecoder`, Web Crypto, `structuredClone`, `fetch`, `XMLHttpRequest` | нет в движке; `fetch` и `XMLHttpRequest` не появятся (песочница) | 0 | — |
| `{{$guid}}`, `{{$randomUUID}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}` в `replaceIn` | да | 0 | 2 |
| faker-переменные (`{{$randomFirstName}}` …) | позже | 0 | — |

## Изменения в базе данных

**DDL нет. Миграции нет.** Таблицы, колонки и индексы не добавляются и не меняются; `PRAGMA user_version` остаётся
`2`; уже применённые версии 1 и 2 не трогаются; файлы `schema.sql` и `schema_v2.sql` не правятся. Новых
SQL-операторов тоже нет — используются существующие `state_get` / `state_set`, `save_environment`
(`INSERT … ON CONFLICT (id) DO UPDATE`) и `update_content` (`UPDATE node SET content`).

Индекс на `node(parent_id)` **не предлагается**: миграции в этом плане нет, а читателя у индекса по-прежнему нет
(`Store::ancestors` идёт вверх по первичному ключу; спека, раздел 5: «Индексов нет»).

Что меняется в **данных** существующих таблиц:

| Где | Что | Кто пишет | Кто читает | Зачем, что сломается без этого |
|---|---|---|---|---|
| `app_state`, новая строка `scripts_allowed` (константа `SCRIPTS_ALLOWED_KEY`, `crates/rqm-runner/src/trust.rs`) | JSON-массив id коллекций, которым разрешено исполнять скрипты: `["<id>", …]` | команда `set_scripts_allowed`; `create_node` (коллекция, созданная в приложении, разрешена сразу); `duplicate_node` (копия разрешённой коллекции разрешена); `delete_node` (убирает id исчезнувших коллекций) | `send_request` (через `load_send_input`); команда `scripts_allowed` | Согласие на скрипты коллекции (спека, разделы 6 и 11). Без строки скрипты чужой коллекции исполнялись бы сразу после импорта |
| `app_state`, строка `settings` | в JSON два новых ключа: `scripts_enabled` (bool), `script_timeout_ms` (целое) | команда `save_settings` | `get_settings`; `send_request` | Общий выключатель скриптов и предел времени скрипта (спека §8: «настраивается») |
| `history.detail` | в JSON два новых ключа: `tests` (массив результатов тестов), `script_errors` (массив ошибок скриптов); в `warnings[].kind` новые значения `scripts_disabled`, `scripts_not_allowed`, `variables_not_saved` | оболочка после отправки | просмотр записи истории | Запись истории показывает, какие тесты прошли и упали в той отправке. Размер ограничен пределами движка: текст тестов — не больше 256 КиБ на фазу (`MAX_TEST_TEXT_BYTES`), ошибок скриптов — не больше одной на скрипт цепочки, каждая до 10 000 символов; запись растёт не больше чем на ~1 МиБ сверх прежних двух тел по 1 МиБ |
| `history.summary` | форма JSON прежняя; у ключа `error_kind` новые значения `script`, `skipped` | оболочка после отправки | список истории | Строка списка истории называет вид неудачи отправки; старые записи не меняются |
| `environment.content` (окружение и строка `globals`) | форма JSON прежняя; значения переменных теперь пишет и скрипт | оболочка после отправки (`apply_variable_changes`) | отправка, редактор окружений, экспорт | `pm.environment.set` / `pm.globals.set`. Комментарий колонки в `schema.sql` это уже называет. Значение, записанное скриптом, — не длиннее 1 МиБ (`MAX_VALUE_BYTES`), всего за одну фазу скриптов — не больше 16 МиБ |
| `node.content` узла-коллекции, ключ `variables` | форма JSON прежняя; значения теперь пишет и скрипт | оболочка после отправки (`apply_variable_changes`) | отправка, редактор коллекции, экспорт | `pm.collectionVariables.set` — единственный `pm.*`, реально встреченный у владельца |

Что происходит с уже записанными данными при обновлении:

- Строки `scripts_allowed` в старой базе нет — ни одной коллекции скрипты не разрешены. Коллекции, импортированные
  или созданные на этапе 1, получают согласие одним нажатием «Разрешить» (вопрос владельцу № 1).
- `settings` без новых ключей читается со значениями по умолчанию `scripts_enabled: true`,
  `script_timeout_ms: 10000`; остальные поля не меняются. Ключи появятся в базе при первом «Сохранить» в настройках.
- Записи `history`, сделанные раньше, читаются: `tests` и `script_errors` — пустые; предупреждение
  `scripts_not_executed` в них остаётся читаемым и показывается с текстом «отправка сделана до появления
  выполнения скриптов».
- Комментарий `node.content` в `schema.sql` («Пишется при импорте и при сохранении редактора») становится
  неполным. Файл не правится (SQLite хранит текст `CREATE TABLE` с комментариями: правка дала бы разный текст
  схемы у старых и новых баз); новое описывается в спеке, раздел 5 (Task 19).

Форма новых значений:

```json
// app_state 'scripts_allowed'
["0b6d5a0e-…", "7c1f…"]
// app_state 'settings' — новые ключи рядом с прежними
{ "timeout_ms": 0, "verify_tls": true, "proxy": { "mode": "system" }, "motion": "normal",
  "scripts_enabled": true, "script_timeout_ms": 10000 }
// history.detail — новые ключи рядом с прежними
{ "tests": [ { "source": { "node_id": "…", "node_name": "Login", "node_kind": "request", "phase": "test" },
               "name": "status is 200", "passed": false, "skipped": false,
               "error": "AssertionError: expected 500 to equal 200" } ],
  "script_errors": [ { "source": { … }, "kind": "exception", "message": "TypeError: x is not a function",
                       "line": 3, "column": 5 } ] }
```

Консоль скриптов в историю **не пишется** (решение № 21): в неё попадают значения, которые скрипт напечатал сам,
в том числе секреты.

## Контракт команд

Новые команды (37 → 39) — каждая в четырёх местах плюс тест, плюс `Api`, `tauriApi`, `fakeApi`:

| № | Команда (Rust, `commands.rs`) | Метод `AppCore` | Метод `Api` (TS) и `invoke` | Задача |
|---|---|---|---|---|
| 1 | `scripts_allowed(core, node_id: String) -> Result<bool, String>` | `scripts_allowed(&self, node_id: &str) -> Result<bool, String>` | `scriptsAllowed(nodeId: string): Promise<boolean>` — `{ node_id }` | 11, 15 |
| 2 | `set_scripts_allowed(core, node_id: String, allowed: bool) -> Result<(), String>` | `set_scripts_allowed(&self, node_id: &str, allowed: bool) -> Result<(), String>` | `setScriptsAllowed(nodeId: string, allowed: boolean): Promise<void>` — `{ node_id, allowed }` | 11, 15 |

`node_id` — любой узел: коллекция, папка или запрос; команда сама находит коллекцию (сам узел или его первого
предка). Неизвестный узел — ошибка.

Изменённые команды (подписи прежние, меняется JSON):

- `send_request` — в `SendResult` пять новых ключей; у `error.kind` новые значения `script`, `skipped`; у
  `warnings[].kind` — `scripts_disabled`, `scripts_not_allowed`, `variables_not_saved` (Task 11). Когда скрипт
  изменил переменные и они записаны (`variable_changes` не пуст), команда отвечает `SendResult`, а не ошибкой
  команды, и при сбое после отправки (решение № 41) — иначе окно не узнало бы о записанных значениях.
- `get_settings`, `save_settings` — в `SettingsDto` ключи `scripts_enabled`, `script_timeout_ms`; `save_settings`
  отклоняет `script_timeout_ms` вне 100…600 000 (Task 12).
- `history_get` — в `HistoryEntry` ключи `tests`, `script_errors` (Task 11).
- `import_files` — у `report[].kind` новое значение `unsupported_script_api` (Task 12).
- `create_node`, `duplicate_node`, `delete_node` — подписи и ответы прежние; попутно ведут список
  `scripts_allowed` (Task 11).

Событий Tauri нет; новых разрешений ядра Tauri в capability нет.

```text
SendResult = { response: ResponseDto|null, error: SendErrorDto|null, warnings: Warning[],        // как раньше
               tests: TestResult[], console: ConsoleLine[], console_truncated: boolean,
               script_errors: ScriptError[], variable_changes: VariableChange[] }
ScriptSource   = { node_id: string, node_name: string, node_kind: "collection"|"folder"|"request",
                   phase: "pre_request"|"test" }
TestResult     = { source: ScriptSource, name: string, passed: boolean, skipped: boolean, error: string|null }
ConsoleLine    = { source: ScriptSource|null,               // null — строка самого приложения (сеть)
                   level: "log"|"info"|"warn"|"error"|"debug"|"network", text: string,
                   at_ms: number }                          // целые мс от начала отправки
ScriptError    = { source: ScriptSource, kind: "exception"|"timeout"|"memory"|"cancelled"|"internal",
                   message: string, line: number|null, column: number|null }   // строка и столбец в тексте скрипта, с 1
VariableChange = { scope: "global"|"collection"|"environment",
                   target_id: string,                       // "globals", id окружения или id узла-коллекции
                   op: "set"|"unset"|"clear", key: string,  // у clear ключ пустой
                   value: string|null }                     // строка только у set
SendErrorDto.kind: … | "script"     // ошибка pre-request скрипта, запрос не отправлен; message — текст ошибки
                     | "skipped"    // pm.execution.skipRequest(): запрос не отправлен, ошибкой не считается
Warning.kind:      … | "scripts_disabled"     // items: []
                     | "scripts_not_allowed"  // items: [имя коллекции]
                     | "variables_not_saved"  // items: [текст ошибки хранилища]
                     | "scripts_not_executed" // только в записях истории этапа 1; новые отправки его не дают
SettingsDto    = { timeout_ms, verify_tls, proxy, motion,   // как раньше
                   scripts_enabled: boolean, script_timeout_ms: number }
HistoryEntry   = { …как раньше…, tests: TestResult[], script_errors: ScriptError[] }
ReportItem.kind: … | "unsupported_script_api"   // detail — имена через ", ": "pm.vault, require('moment')"
```

## Безопасность и пределы

**Когда скрипт исполняется.** Только при Send, только в ядре (окно текст скрипта не исполняет; CSP окна не
меняется), и только если одновременно: (1) в настройках включено «Выполнять скрипты»; (2) коллекции запроса
дано согласие. Согласие — по коллекции: коллекция, созданная в приложении, разрешена сразу; импортированная —
нет, пока пользователь не нажмёт «Разрешить скрипты» (в отчёте импорта, в редакторе коллекции или в
предупреждении после отправки). Без согласия не исполняется ни одна строка, отправка идёт как на этапе 1, с
предупреждением. Запрос, перенесённый в разрешённую коллекцию, исполняет свои скрипты — перенос сделал пользователь.

**Что скрипт может:** читать и менять переменные четырёх областей, включая секретные (иначе не работает получение
токена); читать запрос и в pre-request менять метод, адрес, заголовки и текстовое тело; читать ответ; отправлять
HTTP(S)-запросы на **любой** адрес, включая `localhost` и внутреннюю сеть, с общими cookies, прокси и настройкой
проверки сертификата (`pm.sendRequest`); писать в консоль. Это и есть причина вопроса о согласии — его текст
говорит об этом прямо (Task 15).

**Чего скрипт не может:** читать и писать файлы (API файлов нет; тело-файл и файловые части в `pm.sendRequest`
вырезаются с предупреждением в консоли; pre-request не может сделать тело запроса файлом, binary, form-data или
GraphQL); выходить в сеть иначе, чем через `pm.sendRequest` (нет `fetch`, `XMLHttpRequest`, WebSocket); видеть
процесс и окружение ОС (`process`, `require('fs')`, `os`, `std` отсутствуют); получить от примитивов хоста
что-либо сверх `pm.*` (объект `__host` удаляется из глобальной области до старта скрипта; если скрипт всё же
добрался до примитива подменой встроенных объектов, прямой вызов ничего не даёт — пределы и запреты проверяет
Rust, правило движка № 6); трогать окно; сохранять что-либо, кроме значений переменных; пережить отправку (движок
создаётся на скрипт и уничтожается; исключение — скрипт, брошенный сторожем: его поток считает дальше, но к хосту
обратиться уже не может, см. ниже). `eval` и `new Function` работают — внутри песочницы они не дают новых
возможностей.

**Пределы.** Значение по умолчанию — колонка 2; настраивается только время.

| Предел | Значение | Что происходит при превышении |
|---|---|---|
| Время исполнения JavaScript одного скрипта (ожидание `pm.sendRequest` и таймеров не считается) | 10 с; настройка `script_timeout_ms`, 100…600 000 | ошибка скрипта вида `timeout`; `try/catch` скрипта её не ловит |
| Суммарное ожидание таймеров одним скриптом | 60 с (`TIMER_WAIT_LIMIT`) | ошибка вида `timeout` |
| Ожидание ответа `pm.sendRequest` | общий таймаут запроса из настроек; «без ограничения» — до Cancel | ошибка в callback / отклонённый Promise |
| Память движка на один скрипт | 128 МиБ | ошибка вида `memory` |
| Скрипт, который движок не может прервать (нативный цикл вроде `[].reverse.call({length: 2**53-1})`) | запас 2 с сверх предела времени или после Cancel (`WATCHDOG_GRACE`) | сторож бросает поток: ошибка вида `timeout` (после Cancel — `cancelled`), отправка продолжается как при обычной ошибке скрипта; брошенный поток занимает одно ядро процессора и до 128 МиБ памяти, пока не кончит сам или до закрытия программы |
| Брошенные потоки одновременно | 4 (`MAX_ABANDONED_THREADS`) | скрипты не исполняются: ошибка вида `internal` («сбой движка») с текстом `too many scripts are still running after their time limit; restart the application`, пока брошенный поток не завершится или программу не перезапустят |
| Стек JavaScript | 8 МиБ (`MAX_STACK_BYTES`; поток — 16 МиБ) | `RangeError: Maximum call stack size exceeded` в скрипте |
| Тело ответа, доступное скрипту | 10 МиБ (`BODY_LIMIT`) | `pm.response.text()` / `json()` бросают понятную ошибку, `responseBody` — `undefined` |
| Консоль за одну отправку | 1000 строк и 1 МиБ; строка — до 10 000 символов | лишнее отбрасывается, пометка «вывод обрезан» |
| Тесты за одну фазу | 1000 (`MAX_TESTS`); имя — до 1000 символов, текст ошибки — до 10 000; имена и ошибки вместе — 256 КиБ в UTF-8 (`MAX_TEST_TEXT_BYTES`) | ошибка скрипта; уже записанные тесты остаются |
| Значение переменной, записанное скриптом | 1 МиБ в UTF-8 (`MAX_VALUE_BYTES`) | `set` бросает обычную ошибку, которую скрипт может поймать; значение не меняется |
| Переменные и их изменения за одну фазу | 16 МиБ переменных всех областей, включая local (`MAX_VARIABLES_BYTES`); 10 000 изменений и 16 МиБ журнала (`MAX_CHANGES`, `MAX_CHANGES_BYTES`) | ошибка вида `memory`; `try/catch` её не ловит; изменения до превышения сохраняются |
| `pm.sendRequest` в одном скрипте | 50 (`MAX_SEND_REQUESTS`) | ошибка скрипта |
| Запрос `pm.sendRequest` | 10 МиБ — адрес, заголовки и тело вместе (`MAX_REQUEST_BYTES`); очередь ждущих запросов — 32 МиБ (`MAX_QUEUED_REQUEST_BYTES`) | запрос: ошибка в callback / отклонённый Promise; очередь: ошибка вида `memory` |
| Cancel | в любой момент | скрипт и его сетевой запрос останавливаются; изменения переменных, сделанные до отмены, сохраняются |

«Символ» во всех пределах — скалярное значение Unicode (эмодзи — один символ), а не единица UTF-16. Все пределы
этой таблицы проверяет Rust (правило движка № 6): подмена встроенных объектов скриптом их не снимает.

**Секреты.** Значение секретной переменной скрипту доступно. В консоль оно попадёт, только если скрипт сам его
напечатал; консоль живёт до следующей отправки и в историю не пишется. В историю попадают имена тестов и тексты
их ошибок. Значения, которые скрипт записал в переменные (токены), лежат в базе открыто, как и остальные данные
(спека §11), и уходят в экспорт (решение № 13).

## Решения автора

Каждое владелец может отклонить; после знака «/» — альтернатива.

| № | Решение | Почему / альтернатива |
|---|---|---|
| 1 | Движок — `rquickjs =0.14.0` | Решение спеки; последняя стабильная версия; единственный кандидат с обработчиком прерывания, лимитом памяти и стека; собирается здешним MSVC без bindgen, cmake и clang (`engine.md`, раздел 3) / `boa_engine` — нет лимитов, +16 МБ |
| 2 | Байткод прелюдии и библиотек компилируется **один раз за запуск приложения** (в `OnceLock`), а не при сборке — отступление от спеки §8 | Байткод всегда сделан тем же движком, что его загружает (условие безопасности `unsafe Module::load` выполнено по построению); нет `build.rs`, исполняющего движок; цена — первая отправка со скриптом на несколько миллисекунд дольше / байткод при сборке, как в спеке |
| 3 | Собранная прелюдия лежит в репозитории (`prelude/built/`), `cargo` не зовёт Node; тест сверяет контрольную сумму исходников | `cargo test --workspace` по-прежнему работает без `npm install` / `build.rs` запускает esbuild |
| 4 | Новый `Runtime` и `Context` на **каждый скрипт** (а не один контекст на Send, как в спеке §8); между скриптами переходят только переменные и запрос | Так в Postman: глобальные имена JS от скрипта к скрипту не переходят; предел памяти считается на скрипт; создание — ~0,1 мс / один контекст и обёртка-функция |
| 5 | `pm.expect` — настоящий chai 4.4.1, грузится лениво при первом обращении | Сообщения и цепочки совпадают с Postman буква в букву; скрипты без chai ничего не платят / своё подмножество |
| 6 | `chai-postman` и `postman-collection` не берём: утверждения `pm.response.to.*` — свой плагин chai с теми же именами и текстами сообщений; списки заголовков и параметров — свои маленькие классы | SDK — 522 КБ и тянет faker, iconv-lite, Ajv / взять SDK целиком |
| 7 | Значение переменной: в пределах одной отправки `get` возвращает то, что положил `set` (число, булево, объект — через JSON); в базу пишется строка `String(value)` (`5` → `"5"`, объект → `[object Object]`, `null` → `"null"`, `undefined` → `""`) | В модели одно текстовое значение (спека §5); подстановка `{{var}}` даёт то же, что в Postman. Отличие: в следующей отправке число вернётся строкой / хранить тип — новое поле модели, DTO и экспорта |
| 8 | Скрипт видит только включённые переменные; `set` меняет последнюю включённую строку с таким именем, иначе добавляет новую; `unset` удаляет включённые строки с именем; `clear` удаляет **только включённые** (Postman удаляет и выключенные); `toObject` — только включённые | Правила `postman-collection` для `get/set/unset`; для `clear` и `toObject` — осторожнее, выключенная строка скрипту невидима и им не стирается / точь-в-точь Postman |
| 9 | `pm.environment.set` без выбранного окружения: значение живёт до конца отправки, в базу не пишется, в консоли предупреждение | Писать некуда; молчаливая потеря хуже / ошибка скрипта |
| 10 | Ошибка pre-request скрипта любого уровня: остальные pre-request не исполняются, запрос не отправляется, test не исполняются | Так ведёт себя приложение Postman (по памяти; движок Newman без `--bail` отправил бы) / отправлять с предупреждением |
| 11 | Ошибка test-скрипта: ответ показан, следующие test-скрипты цепочки исполняются | Так делает движок Postman / остановить цепочку |
| 12 | Изменения переменных записываются в базу **один раз, когда отправка завершилась** (в том числе ошибкой или отменой), дельтой поверх свежего чтения | Спека §8 говорит «сразу»; запись из потока скрипта потребовала бы доступа к базе из ядра во время `.await`. Сделанное до ошибки сохраняется — пометка **[проверить]** спеки закрыта по исходникам движка Postman / писать после каждого скрипта |
| 13 | Значение, записанное скриптом, — то же единственное значение, что уходит в экспорт (пары initial/current нет) | Решение спеки §5; иначе — изменение модели / пара значений (спека относит к этапу 7) |
| 14 | Переменные коллекции остаются частью черновика вкладки коллекции; окно применяет дельту скрипта и к сохранённой копии, и к черновику открытой вкладки | Спека §9 этого случая не видела («не конфликтует с черновиком» верно только для окружений). Несохранённые правки Auth, Scripts, описания целы / сохранять переменные коллекции сразу, как окружения |
| 15 | Перед отправкой окно дописывает несохранённые правки вкладок активного окружения и Globals; правки вкладки коллекции и папки — нет (нужен Ctrl+S) | Скрипт должен видеть то, что на экране; черновик коллекции по спеке сохраняется только Ctrl+S / передавать черновики предков в `send_request` |
| 16 | Согласие хранится в `app_state` (`scripts_allowed` — список id), а не полем в содержимом коллекции | Действует сразу, не входит в черновик и Ctrl+S, не едет через `get_node` / `save_node_content`, не задевает экспорт. Для пользователя это та же «настройка коллекции» из спеки §6 / поле `CollectionContent` |
| 17 | Вопрос о согласии — не отдельное окно, а блок в отчёте импорта с кнопкой «Разрешить скрипты» у каждой коллекции со скриптами | Один диалог вместо цепочки; отказ — просто закрыть отчёт / модальный вопрос на каждую коллекцию |
| 18 | `pm.sendRequest` одного скрипта исполняются по очереди, в порядке вызова | Результат тот же, что при параллельной отправке, кроме времени; проще и без гонок / параллельно |
| 19 | Первая необработанная ошибка в асинхронной части (callback таймера или `pm.sendRequest`, необработанный отклонённый Promise) останавливает скрипт | Предсказуемо; в Postman остальные таймеры продолжают / как в Postman |
| 20 | Консоль приходит целиком вместе с ответом, событий «ядро → окно» нет | В проекте нет ни одного события; живая консоль нужна только для долгих скриптов / `tauri::ipc::Channel` |
| 21 | В истории хранятся тесты и ошибки скриптов; консоль — нет. Объём тестов в записи ограничен пределом движка: имена и тексты ошибок тестов одной фазы — не больше 256 КиБ (`MAX_TEST_TEXT_BYTES`), превышение — ошибка скрипта | Секреты в выводе; рост базы (история уже до 1 МиБ на тело, хранится 500 записей; без предела одна запись с тестами могла бы занять десятки мегабайт) / окно показывает все тесты, история — усечённый список с пометкой (два предела и новый ключ `tests_truncated`) |
| 22 | Консоль — вкладка панели ответа запроса (спека §9), не общая панель окна; в ней же строки сети: основной запрос и каждый `pm.sendRequest` | Запросы скрипта иначе нигде не видны (в историю они не пишутся) / общая панель окна |
| 23 | Ошибка скрипта показывается текстом «узел, вид скрипта, строка N, столбец M»; подсветки строки в редакторе и перехода к ней нет | Без новой зависимости (`@codemirror/lint`) и без внешнего управления разделами редакторов / `Decoration.line` и переход |
| 24 | Автодополнение — свой источник по статическому списку `ui/src/lib/scriptApi.ts`; тест сверяет список с таблицей ядра `prelude/src/api.json` | `scopeCompletionSource` из `lang-javascript` в живом редакторе не проверялся / он |
| 25 | Отчёт импорта называет неподдержанное поиском по тексту (спека §6): совпадение в комментарии или строке тоже попадёт в отчёт | Просто и не пропускает; лишняя запись безвредна / разбор JS |
| 26 | Тело ответа для скрипта — UTF-8 с заменой битых байт, BOM срезается; другие кодировки не поддержаны | В ядре нет перекодировки (окно делает то же) / `encoding_rs` |
| 27 | `pm.response.status` — стандартная фраза по коду (`HttpResponse.status_text`), а не то, что прислал сервер | Так устроен `rqm-http`; `to.have.status("OK")` работает для стандартных кодов / менять `rqm-http` |
| 28 | В `pm.sendRequest` `{{переменные}}` **не подставляются** — адрес, заголовки и тело уходят как дал скрипт; скрипты и auth коллекции к нему не применяются; он не пишется в историю | Про подстановку — как в Postman, сверено по исходникам (`postman-runtime` 7.56.3, `event.command.js`: запрос скрипта идёт сразу в `httprequest`, минуя `resolveVariables`; `postman-sandbox` 6.9.0, `pmapi.js`). Подстановка меняла бы байты, уходящие на чужой сервер (шаблон `Hello {{name}}` в теле; значение из ответа, вставленное в следующий запрос). Нужна подстановка — `pm.variables.replaceIn`. Про auth и историю — по памяти / подставлять |
| 29 | Событие `disabled` у скрипта и второе событие того же вида в файле Postman по-прежнему не читаются импортом | В образце владельца нет; правка `rqm-postman` — вне этапа / учитывать `disabled` |
| 30 | Предел времени — на скрипт, не на цепочку; предел памяти и стека — константы | Спека §8: «на скрипт», настраиваемым называет только время / настройки на всё |
| 31 | Цели скорости для отправки со скриптами (в спеке их нет): типичный скрипт — накладные расходы ≤ 20 мс сверх сети; первый запрос со скриптом после запуска — ≤ 50 мс; запрос без скриптов — как раньше | Та же планка, что у запроса без скриптов; первая отправка компилирует прелюдию и chai / другие числа |
| 32 | Тексты лицензий встроенных библиотек (QuickJS-NG, rquickjs, chai и 7 его зависимостей, lodash; тексты сообщений утверждений — chai-postman, Apache-2.0) — файл `THIRD-PARTY-LICENSES.txt` в корне репозитория, попадает в установщик как ресурс | MIT требует текста лицензии в поставке. Из добавленного планом в бинарник попадает ещё `allocator-api2` (MIT OR Apache-2.0, через `hashbrown`) — его текст тоже в файле; `relative-path` в сборку не входит / экран «О программе» |
| 33 | Пробник `spike/script-probe/` удаляется в Task 19 | Его заменяет `rqm-script`; прецедент — HTTP-пробник удалён после этапа 1A / оставить |
| 34 | При отмене окно показывает «Запрос отменён» сразу и не ждёт ядро (как сейчас); тесты и консоль отменённой отправки не показываются, изменения переменных применяются | Меньше всего меняет нынешнее поведение / дорисовывать консоль в «отменён» |
| 35 | Анимация «счётчик тестов» (спека §9) — только проявление значка (`opacity`, `--dur-2`) | «Анимации сверх простых» исключены постановкой плана 2 / счётчик с перелистыванием |
| 36 | `pm.execution.skipRequest()` делается в первой версии — **отступление от спеки §8**, которая относит его к «позже, вместе с Runner»; у владельца счётчик 0. По той же причине отступление: `pm.iterationData` (пустые `get / has / toObject`), `pm.visualizer.set / clear` и `pm.execution.setNextRequest` / `postman.setNextRequest` (ничего не делают) есть в первой версии, хотя спека (§8, «Позже») относит их к «позже» | `skipRequest` не требует Runner (он отменяет одну отправку) и дёшев; пустые `iterationData` и `visualizer` не дают старым скриптам падать на `undefined`. Цена `skipRequest`: вид ошибки `skipped` в DTO, в истории и в текстах окна / `skipRequest` — «позже» (бросает `… is not supported in RequestMaster`) или пустая функция: тогда из Task 8, 11, 14 и из случая 19 Task 17 убирается `SendError::Skipped`, `Report.skip_request`, `skipped`; `iterationData` и `visualizer` — «позже» (из Task 9 убираются две строки таблицы) |
| 37 | Скрипт, который движок не может прервать, бросается сторожем через 2 с сверх предела времени (или после Cancel): отправка получает ошибку `timeout` / `cancelled`, поток доживает сам и к хосту больше не обращается. Одновременно брошенных потоков — не больше 4; дальше скрипты не исполняются до перезапуска программы (или пока брошенный поток не кончит) | Обработчик прерывания QuickJS не вызывается в части нативных циклов (проверено опытом: девять выражений вида `[].reverse.call({length: 2**53-1})` не остановились за 20 с); убить поток ОС безопасно нельзя. Без сторожа одна строка скрипта навсегда вешает отправку. Цена: брошенный поток занимает ядро процессора и до 128 МиБ / другие числа запаса и порога; понижение приоритета брошенного потока (нужен `SetThreadPriority` через `unsafe` или новую зависимость — не делается, только предложение) |
| 38 | Пределы на сторону хоста (в спеке их нет): значение переменной — 1 МиБ; переменные всех областей за фазу — 16 МиБ; журнал изменений — 10 000 записей и 16 МиБ; запрос `pm.sendRequest` — 10 МиБ, очередь — 32 МиБ; текст тестов фазы — 256 КиБ | Предел 128 МиБ действует только на кучу QuickJS; без этих пределов `while (true) pm.globals.set('k', 'x'.repeat(1 << 20))` за 10 с набирает гигабайты в куче Rust, а отказ аллокатора Rust — это закрытие программы. Заодно ограничен размер того, что скрипт пишет в базу и в окно / другие числа |
| 39 | Одиночный суррогат в тексте, уходящем из скрипта (консоль, ключи и значения переменных, имена и ошибки тестов, запрос, `pm.sendRequest`, текст исключения), заменяется на U+FFFD; внутри скрипта строка остаётся как была | Такая строка не превращается в строку Rust — `console.log`, `pm.environment.set` и `pm.test` падали бы с непонятной ошибкой преобразования; в Postman они работают. Встречается в данных (`JSON.parse('"\\ud83d"')`) и при обрезке текста посреди эмодзи / ошибка скрипта |
| 40 | В test-скрипте `pm.request` (и старый `request`) — запрос, который ушёл в сеть: переменные подставлены, заголовок авторизации есть; автоматических заголовков (`User-Agent`, `Host`, `Content-Length` …) в нём нет | Так в Postman (сверено по `postman-runtime` 7.56.3, `item.command.js`: `ctxTemplate.request = request`); скрипт `JSON.parse(pm.request.body.raw)` иначе падал бы на `{{…}}`. Автоматические заголовки добавляет HTTP-клиент, ядро их до отправки не знает / показывать и их (нужно читать `response.sent.headers`) |
| 41 | Если скрипт изменил переменные и они записаны, команда `send_request` всегда отвечает `SendResult` с этой дельтой, даже когда после отправки случился сбой: не прочиталось тело ответа → `error.kind = "io"`; не сохранились cookies после неудачной отправки → сбой игнорируется, как уже делается после удачной. Отправки без изменений переменных ведут себя как раньше | Иначе окно не получает дельту: открытая вкладка окружения остаётся со старыми строками и следующей автозаписью затирает значения скрипта / при отказе команды окно перечитывает открытые вкладки окружений (теряются правки, сделанные во время отправки; вкладка коллекции не обновляется) |

## Вопросы владельцу

Нужны до начала исполнения; по каждому — моя рекомендация.

1. **Коллекции, которые уже лежат в базе.** После обновления их скрипты не исполняются, пока вы не нажмёте
   «Разрешить» (один раз на коллекцию; кнопка — прямо в предупреждении после Send).
   Варианты: (а) так — **рекомендую**: правило одно для всех импортированных коллекций, данных в базе не трогаем;
   (б) при первом запуске новой сборки разрешить все существующие коллекции разом — тогда нужен разовый DML
   (запись их id в `scripts_allowed`) и отметка «уже сделано».
2. **Общий выключатель «Выполнять скрипты» в настройках** (в спеке его нет). Варианты: (а) делать —
   **рекомендую**: одна галочка, позволяет быстро отправить запрос «как на этапе 1»; (б) не делать — остаётся
   только согласие по коллекциям (тогда из Task 10, 12, 15 убирается `scripts_enabled`).
3. **Проверки в живом Postman и образцы.** Часть поведения приложения Postman известна только по памяти (решения
   № 9, 10 и часть № 28 про auth и историю; подстановка в `pm.sendRequest` и запрос в test-скрипте сверены по
   исходникам и проверки не требуют). Если есть 10 минут — семь коротких проверок из `postman-api.md`, раздел
   15; и ещё 2–4 коллекции
   с окружением (или прогон счётчика, который печатает только имена API). План от ответа не зависит; ответ может
   поднять что-то из «позже» в таблице API. Вариант: пропустить — **допустимо**.
4. **Ветка.** Работать в текущей `feature/stage1-part3` (так велит общее правило) или завести от неё
   `feature/stage2-scripts`. **Рекомендую** отдельную ветку: этап 1 можно будет влить независимо.

## Не входит в план

- По спеке §2 и §12: Collection Runner, CLI, `setNextRequest` как действие, `pm.iterationData` с данными,
  визуализатор, vault, пакеты библиотеки Postman (`pm.require`, `script.packages` только сохраняются), gRPC,
  WebSocket, командная работа, пара initial/current.
- Всё, что в таблице API помечено «позже», включая `crypto-js`, `moment`, `ajv`, `uuid`, `pm.cookies`, `Buffer`.
- Живая консоль (события), общая панель консоли окна, консоль в истории, счётчик тестов в **списке** истории,
  подсветка строки ошибки в редакторе и переход к ней, подсветка `{{var}}` в скриптах, маскирование секретов в
  консоли, декодирование не-UTF-8 ответов, события прогресса запроса.
- Нераспределённые требования этапа 1 (автозаголовки группой, просмотр cookie jar, Ctrl+T, виртуализация таблиц,
  редактор GraphQL, подсветка XML/HTML, общие настройки редиректов) — остаются нераспределёнными.
- Предлагаю от себя (не делается): общий файл лицензий для всех зависимостей приложения, а не только для
  добавленных этим планом; кнопка «Скопировать вывод» в консоли; правка `parse_scripts` под `disabled` событие.
- Остаются отложенными замечания журналов: W-a, W-c, W-d, T-c, T-l, T-n, T-o, T-r, P-j, P-n, P-x и остальные из
  списка плана 2. Молчаливая остановка подстановки по пределу 1 МиБ (`MAX_GROWTH`) касается и `replaceIn` —
  остаётся как есть.

---

## Карта файлов

```
crates/rqm-script/                 новый crate: движок и песочница
  src/lib.rs                       публичные типы (Job, Report, Limits, …), константы пределов, запреты clippy
  src/engine.rs                    поток, сторож, Runtime/Context на скрипт, обёртка скрипта, цикл заданий, чтение ошибок
  src/budget.rs                    секундомер исполнения JS (пауза на время ожидания хоста), срок для сторожа, отмена
  src/text.rs                      обрезка по границе символа, чтение строк JS с одиночным суррогатом
  src/bundle.rs                    собранные JS-файлы, байткод в OnceLock, загрузка в контекст
  src/host.rs                      примитивы хоста (объект __host): консоль, переменные, тело ответа, таймеры, сеть
  src/variables.rs                 Variables, правила set/unset/clear, журнал изменений, Resolver
  src/api.rs                       таблица API (api.json) и поиск неподдержанного в тексте скрипта
  prelude/package.json, package-lock.json, build.mjs
  prelude/src/*.js, api.json       исходники pm.*: index, console, variables, lists, response, test, legacy,
                                   chai-entry, assertions, async, send-request, request, require, stubs
  prelude/built/{prelude,chai,lodash}.js   собранное esbuild; коммитится
  tests/*.rs                       engine, watchdog, watchdog_cap, bundle, console, variables, tamper, response,
                                   tests_api, expect, async_run, send_request, request, api, perf, perf_cold
crates/rqm-runner/src/
  scripts.rs                       цепочка скриптов узлов, сборка Job, перевод отчёта движка в ScriptsReport
  script_host.rs                   rqm_script::Host поверх канала: pm.sendRequest -> HttpEngine
  variable_changes.rs              запись изменений переменных в хранилище (одно правило на все области)
  trust.rs                         ключ scripts_allowed
  send.rs, prepare.rs, lib.rs      конвейер pre-request -> prepare -> HTTP -> test
src-tauri/src/scripts.rs           DTO скриптов, команды согласия, запись в отчёт импорта
ui/src/lib/variableSync.ts         применение дельты переменных к открытым вкладкам
ui/src/lib/scriptApi.ts, lib/codemirror/scriptCompletions.ts     подсказки pm.*
ui/src/components/TestResults.svelte, ConsoleView.svelte          вкладки Tests и Console
```

---

### Task 1: `rqm-script` — движок и пределы

**Files:**
- Modify: `Cargo.toml` (корень), `Cargo.lock`
- Create: `crates/rqm-script/Cargo.toml`, `crates/rqm-script/src/lib.rs`, `src/engine.rs`, `src/budget.rs`,
  `src/text.rs`
- Test: `crates/rqm-script/tests/engine.rs`, `tests/watchdog.rs`, `tests/watchdog_cap.rs`

**Interfaces:**
- Consumes: `rquickjs` 0.14.0 (`Runtime::new`, `set_memory_limit`, `set_max_stack_size`, `set_interrupt_handler`,
  `Context::full`, `Context::with`, `Ctx::eval_with_options`, `EvalOptions { strict, filename, .. }` — структура
  `non_exhaustive`, создаётся через `Default`; `Ctx::execute_pending_job`, `Promise::state / result`, `Ctx::catch`);
  `tokio_util::sync::CancellationToken`. Образец секундомера и цикла — `spike/script-probe/src/lib.rs` (только как
  образец: его `unwrap` и BOM в начале файлов не переносить).
- Produces:

```rust
// crates/rqm-script/src/lib.rs — все типы отчёта: #[derive(Debug, Clone, PartialEq)]
/// Stack of the thread the scripts run on. Must stay at least twice `MAX_STACK_BYTES`.
pub const THREAD_STACK_BYTES: usize = 16 * 1024 * 1024;
/// Bounds `Limits::stack_bytes` is clamped to. Above 16 MiB rquickjs switches the stack check off; a limit
/// equal to the stack of the thread never fires before the real overflow, which kills the process — so the
/// upper bound stays at half of `THREAD_STACK_BYTES`.
pub const MIN_STACK_BYTES: usize = 1024 * 1024;
pub const MAX_STACK_BYTES: usize = 8 * 1024 * 1024;
const _: () = assert!(MAX_STACK_BYTES * 2 <= THREAD_STACK_BYTES);
/// How long `run` waits past the time limit, or past a cancel, for a script the engine cannot interrupt.
pub const WATCHDOG_GRACE: Duration = Duration::from_secs(2);
/// The most script threads that may still be running after `run` gave up on them.
pub const MAX_ABANDONED_THREADS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// JavaScript execution time of one script; waiting for the host is not counted.
    pub time: Duration,
    pub memory_bytes: usize,
    pub stack_bytes: usize,
    /// How long past `time`, or past a cancel, the watchdog waits before it abandons the thread.
    pub grace: Duration,
}
impl Default for Limits { /* 10 s, 128 MiB, 8 MiB, WATCHDOG_GRACE */ }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase { PreRequest, Test }

/// The scripts of one phase of one send, in the order they run.
#[derive(Debug, Clone)]
pub struct Job { pub phase: Phase, pub scripts: Vec<String>, pub limits: Limits,
                 /// The start of the send: console lines are timed from it.
                 pub epoch: Instant }
impl Job { /// Default limits, `epoch` = now, everything later tasks add — empty.
           pub fn new(phase: Phase, scripts: Vec<String>) -> Self; }

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report { pub errors: Vec<ScriptError> }

pub struct ScriptError { /// Index in `Job::scripts`.
                         pub script: usize, pub kind: ScriptErrorKind, pub message: String,
                         /// Line and column in the text of the script as the editor numbers them, from 1.
                         pub line: Option<u32>, pub column: Option<u32> }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptErrorKind { Exception, Timeout, Memory, Cancelled, Internal }

/// Runs the scripts one after another on a thread of its own (stack `THREAD_STACK_BYTES`) and blocks until
/// they have finished — or until the watchdog gives up on a script the engine cannot interrupt: never longer
/// than the time the scripts are allowed plus `Limits::grace` per script. An empty job starts nothing.
pub fn run(job: Job, cancel: &CancellationToken) -> Report;
/// Script threads `run` gave up on that have not ended yet. For tests and measurements.
pub fn abandoned_threads() -> usize;
```

**Правила:**
- Корневой `Cargo.toml`: в `[workspace.dependencies]` — `rquickjs = "=0.14.0"` с комментарием «Script engine of
  `rqm-script`: QuickJS-NG 0.16.2. Default features only…»; новый раздел
  `[profile.dev.package.rquickjs-sys] opt-level = 3` с комментарием (неоптимизированный C-код QuickJS даёт вчетверо
  более глубокие кадры стека и в 2–4 раза медленнее — тесты пределов видели бы не то, что продукт).
- `run` создаёт **отсоединённый** поток `std::thread::Builder::new().name("rqm-script").stack_size(
  THREAD_STACK_BYTES).spawn(…)` (не scoped: брошенный поток должен пережить `run`); `job` и копия токена уходят в
  поток владением. Общее с потоком — `Arc`: состояние отчёта под `Mutex<Option<…>>` (правило движка № 9), флаг
  `abandoned: AtomicBool`, срок для сторожа. Поток не создался → один `ScriptError { script: 0, kind: Internal,
  message: "script thread failed" }` (достижимо в любом профиле). Поток завершился, не сдав отчёт (паника) → то
  же — но только в dev- и test-профиле: в release `panic = "abort"`, и паника закрывает приложение (правило
  движка № 3).
- **Сторож.** Поток скрипта публикует срок: пока исполняется JavaScript скрипта — «сейчас + остаток секундомера +
  `limits.grace`»; пока секундомер стоит (ожидание хоста и таймеров, чтение тела ответа, загрузка bundle — Task 2,
  3, 6) — срока нет. `run` ждёт конец потока через `std::sync::mpsc::Receiver::recv_timeout` шагами не длиннее
  20 мс и бросает поток, когда (а) опубликованный срок прошёл, или (б) токен отменён дольше `limits.grace` — в
  любом состоянии потока. Бросить — значит: выставить `abandoned`; забрать состояние отчёта из-под замка, оставив
  `None` (поздние записи брошенного потока молча отбрасываются); добавить ошибку текущего скрипта — токен отменён
  → `Cancelled`, `cancelled`; иначе `Timeout` с тем же текстом, что у обычного таймаута; увеличить счётчик
  брошенных потоков (`static AtomicUsize`); вернуть отчёт. Остальные скрипты этого `Job` не исполняются и в
  `Phase::Test`: поток занят.
- Брошенный поток: обработчик прерывания возвращает `true` и по флагу `abandoned` — поток кончается на первой же
  прерываемой точке и освобождает свой `Runtime`; каждый примитив хоста (Task 2 и дальше) первым делом проверяет
  `abandoned` и бросает исключение JS, ничего не сделав; цикл заданий (Task 6) при `abandoned` не зовёт хост и
  не ждёт таймеры. Когда брошенный поток всё же завершился, он уменьшает счётчик.
- `run` в самом начале: счётчик брошенных потоков ≥ `MAX_ABANDONED_THREADS` → поток не создаётся, отчёт — один
  `ScriptError { script: 0, kind: Internal, message: "too many scripts are still running after their time limit;
  restart the application" }`.
- На каждый скрипт — новый `Runtime` (`set_memory_limit(limits.memory_bytes)`,
  `set_max_stack_size(limits.stack_bytes.clamp(MIN_STACK_BYTES, MAX_STACK_BYTES))`, обработчик прерывания «токен
  отменён, **или** секундомер скрипта истёк, **или** выставлен `abandoned`» — Task 2 добавит к нему флаг предела
  памяти хоста) и новый `Context::full`; вход в `Context::with` — один на скрипт.
- Скрипт исполняется как `format!("(async()=>{{{code}\n}})()")`, `strict = false`, `filename = "script.js"`.
  После `eval` ядро само крутит очередь заданий, пока итоговый Promise не завершится. Promise ждёт, а заданий
  нет → ошибка `Exception` с текстом `the script never finishes: its promise is pending and nothing can settle it`
  (Task 6 добавит в этот цикл таймеры и обращения к хосту).
- Чтение ошибки: `name`, `message`, `stack` читаются как обычные свойства (ошибка chai — не нативная). Текст —
  `"{name}: {message}"`; без `name` — `message`; брошено не объектом — его строковая форма. Каждая строка
  читается как `rquickjs::String`; если `to_string()` отказал (одиночный суррогат: `throw new Error('\ud800')`),
  строка пропускается через `String.prototype.toWellFormed`, который Rust взял из контекста **до** исполнения
  текста скрипта (переменная живёт внутри того же `Context::with` — правило № 2 не нарушено), и читается снова;
  не вышло и так — текст `<unreadable text>`. Ошибка скрипта не теряется и не становится `Internal`. Текст
  обрезается помощником `text::clip` до 10 000 символов (правило движка № 8). Строка и столбец — из первой
  строки `stack`, где файл `script.js`; форматы `at <имя> (script.js:L:C)` и `at script.js:L:C`. На строке 1 из столбца вычитается 11 (длина `(async()=>{`), не меньше 1.
  Строка больше числа строк скрипта (синтаксическая ошибка в самом конце текста попадает на строку обёртки —
  проверено опытом: `"x = 1;\nif ("` даёт `script.js:3:1`) заменяется последней строкой скрипта, столбец — `None`.
- Вид ошибки — по флагам, в таком порядке: токен отменён → `Cancelled`, текст `cancelled`; секундомер истёк →
  `Timeout`, текст `the script ran longer than {N} ms`; флаг предела памяти хоста (Task 2) → `Memory` с текстом
  примитива; `Error::Allocation` либо `InternalError` с `out of memory` → `Memory`, текст `the script used more
  than {N} MiB of memory`; иначе `Exception`.
- `src/text.rs`: `pub(crate) fn clip(text: &str, max_chars: usize) -> &str` — префикс не длиннее `max_chars`
  скалярных значений, по границе символа (`char_indices().nth(max_chars)`); чтение строки JS с запасным путём
  через `toWellFormed`. Других мест, где текст режется, в crate нет.
- Что делает ошибка с цепочкой: в `Phase::PreRequest` любая ошибка останавливает цепочку; в `Phase::Test` —
  только `Cancelled`, остальные виды — следующий скрипт исполняется. Токен, отменённый до старта скрипта, — ошибка
  `Cancelled` этого скрипта без исполнения.

- [ ] **Step 1: Падающие тесты** (`tests/engine.rs`; помощник `run_scripts(phase, &[&str], limits) -> Report`):
  - `scripts_run_in_order_and_do_not_share_globals` — `["leak = 1; var v = 2; function f() {}; const dup = 1",
    "const dup = 2; if (typeof leak !== 'undefined' || typeof v !== 'undefined' || typeof f !== 'undefined') throw
    new Error('leaked')"]` → `errors` пуст;
  - `sloppy_mode_top_level_await_and_return_work` — `"with ({ a: 1 }) { if (a !== 1) throw new Error('with') }\n
    const v = await Promise.resolve(2);\nif (v !== 2) throw new Error('await');\nreturn;\nthrow new Error('after
    return')"` и `"// only a comment"` → `errors` пуст;
  - `error_names_the_line_and_column_of_the_editor` — таблица: `"\n\nundefinedFn(1)"` → `Exception`,
    `ReferenceError: undefinedFn is not defined`, строка 3, столбец 1; `"  boom()"` → строка 1, столбец 3;
    `"const a = 1;\nconst = ;"` → текст `SyntaxError: variable name expected`, строка 2; `"x = 1;\nif ("` →
    начинается с `SyntaxError`, строка 2, столбца нет; `"a = 1;\r\nboom()"` → строка 2, столбец 1;
    `"throw 'plain'"` → текст `plain`, строки нет; `"function f() { null.x }\nf()"` → начинается с `TypeError`,
    строка 1; `"await Promise.reject(new Error('late'))"` → `Error: late`; `"await new Promise(() => {})"` →
    `Exception`, текст содержит `never finishes`;
  - `pre_request_stops_at_the_first_error_and_tests_go_on` — `["throw new Error('a')", "throw new Error('b')"]`:
    `PreRequest` → одна ошибка, `script == 0`; `Test` → две, `script` 0 и 1;
  - `infinite_loop_is_a_timeout` — предел 200 мс, `["try { while (true) {} } catch (e) {} finally {}", "throw new
    Error('next')"]`, `Test` → `errors[0].kind == Timeout`, текст `the script ran longer than 200 ms`,
    `errors[1].message == "Error: next"`, всё вместе быстрее 3 с;
  - `catastrophic_regex_is_a_timeout` — `"/(a+)+$/.test('a'.repeat(40) + '!')"`, 200 мс → `Timeout`;
  - `time_limit_is_per_script` — три скрипта `"const t = Date.now(); while (Date.now() - t < 120) {}"`, предел
    300 мс → `errors` пуст;
  - `memory_limit_is_a_memory_error` — 16 МиБ, `"const a = []; while (true) a.push('x'.repeat(1 << 20) +
    a.length)"` → `Memory`;
  - `infinite_recursion_is_an_error_of_the_script` — `"function f() { f() } f()"` → `Exception`, `RangeError:
    Maximum call stack size exceeded`; то же при `Limits { stack_bytes: 64 << 20, .. }` и при `stack_bytes: 16 <<
    20` (значение зажимается до `MAX_STACK_BYTES`; без зажима второй случай убивает процесс теста);
    `"JSON.parse('['.repeat(100000))"` → `Exception`; после каждого случая следующий `run` работает;
  - `error_text_is_read_whatever_it_holds` — `"throw new Error('a\ud800b')"` → `Exception`, текст `Error:
    a\u{FFFD}b`; `"throw new Error('я'.repeat(20000))"` → `Exception`, текст ровно из 10 000 символов
    (`chars().count()`), начинается с `Error: я`; то же с `'😀'.repeat(20000)` — 10 000 символов, последний —
    целый эмодзи; `"throw '\ud83d'"` → текст `\u{FFFD}`; ни в одном случае вид не `Internal`;
  - `cancel_stops_a_running_script` — предел 60 с, `["while (true) {}", "throw new Error('next')"]`, другой поток
    отменяет токен через 100 мс → в обеих фазах одна ошибка `Cancelled`, быстрее 2 с;
  - `a_cancelled_token_runs_nothing` — токен отменён заранее → `[ScriptError { script: 0, kind: Cancelled, .. }]`;
  - `no_ambient_io_globals` — скрипт бросает ошибку, если хоть одно из `fetch`, `XMLHttpRequest`, `WebSocket`,
    `process`, `require`, `module`, `std`, `os` не `undefined` → `errors` пуст;
  - `empty_job_starts_nothing` — `scripts` пуст → `Report::default()`.

  Помощник `run_scripts` всех интеграционных тестов `rqm-script` (и этой, и следующих задач) сам утверждает, что
  в отчёте нет ошибок вида `Internal`; тест, который проверяет именно `Internal`, зовёт `run` напрямую.

  Сторож — в двух отдельных файлах, **по одному `#[test]` в каждом**, шаги идут по порядку: счётчик брошенных
  потоков общий на процесс, а каждый тестовый файл — свой процесс. Скрипт, который не кончается никогда, держит
  ядро процессора до конца процесса теста — поэтому таких случаев мало, и в остальных файлах тестов их нет
  (там, где нужен брошенный поток, берётся конечный `(10n ** 300000n).toString()` — около 2,4 с на этой машине).
  - `tests/watchdog.rs`, тест `native_loops_end_by_the_watchdog` — предел времени 200 мс, запас по умолчанию;
    по очереди `"new Array(2**32-1).join('')"`, `"[].reverse.call({length: 2**53-1})"`, `"[].splice.call({length:
    2**53-1}, 0, 1)"` → каждый раз одна ошибка `Timeout` с текстом `the script ran longer than 200 ms` не позже
    чем через 200 мс + `WATCHDOG_GRACE` + 1 с; после каждого `run("1")` работает; в конце `abandoned_threads()
    == 3`. (Механизм от выражения не зависит — трёх достаточно; остальные из правила движка № 10 не
    повторяются.)
  - `tests/watchdog_cap.rs`, тест `abandoned_threads_are_counted_and_capped` — (1) `Limits { time: 100 мс,
    grace: 100 мс, .. }`, `"(10n ** 300000n).toString()"` → `Timeout` быстрее 1,5 с; затем ожидание (до 30 с),
    пока `abandoned_threads()` не станет `0` — конечный брошенный поток уменьшил счётчик; (2) предел 60 с,
    `grace` 300 мс, `"[].reverse.call({length: 2**53-1})"`, отмена токена через 100 мс → одна ошибка `Cancelled`
    не позже чем через 1,5 с; `abandoned_threads() == 1`; (3) ещё три раза то же выражение с пределом 100 мс и
    запасом 100 мс, без отмены → каждый раз `Timeout`, после третьего `abandoned_threads() == 4`; следующий `run("1")` → ровно одна ошибка `Internal` с текстом `too many
    scripts are still running after their time limit; restart the application`, скрипт не исполнялся.
- [ ] **Step 2: Запустить** `cargo test -p rqm-script` — crate не собирается или тесты падают.
- [ ] **Step 3: Реализовать** по блоку Interfaces и правилам. Проверить `cargo tree -p rqm-script -i tauri` — пакета
  нет; `cargo tree -p rqm-script | findstr /i "aws-lc native-tls openssl"` — пусто. В `Cargo.lock` появятся
  `rquickjs`, `rquickjs-core`, `rquickjs-sys`, `allocator-api2` и запись `relative-path` 2.0.3 — последняя в
  сборку не входит (`cargo tree -p rqm-script` её не показывает); это ожидаемо, не «лишний» пакет.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные; в отчёт — время холодной сборки `rquickjs-sys`.
- [ ] **Step 5: Commit** — «Этап 2: crate rqm-script — движок QuickJS, пределы времени, памяти и стека, отмена».

---

### Task 2: Прелюдия — сборка, `console`, переменные, `pm.info`

**Files:**
- Create: `crates/rqm-script/prelude/package.json`, `package-lock.json`, `build.mjs`, `prelude/src/index.js`,
  `prelude/src/console.js`, `prelude/src/variables.js`, `prelude/built/prelude.js`
- Create: `crates/rqm-script/src/bundle.rs`, `src/host.rs`, `src/variables.rs`
- Modify: `crates/rqm-script/Cargo.toml` (+ `rqm-vars`, `serde`, `serde_json`), `src/lib.rs`, `src/engine.rs`
- Test: `crates/rqm-script/tests/bundle.rs`, `tests/console.rs`, `tests/variables.rs`, `tests/tamper.rs`;
  модульные тесты примитивов в `src/host.rs`, `src/variables.rs`

**Interfaces:**
- Consumes: Task 1 (в том числе флаг `abandoned`, состояние отчёта под замком, `text::clip`);
  `rqm_vars::{Resolver, Scope, ScopeKind}` (`Resolver::new(scopes)` — от широкой области к
  узкой, `substitute(&str) -> Substituted { text, .. }` с системными динамическими значениями);
  `rquickjs::Module::{declare, write, load, eval}`, `WriteOptions { strip_source, strip_debug, .. }`.
- Produces:

```rust
pub const CONSOLE_MAX_LINES: usize = 1000;
pub const CONSOLE_MAX_BYTES: usize = 1_048_576;
pub const CONSOLE_LINE_CHARS: usize = 10_000;
/// The longest value (UTF-8 bytes of `text`, and of `typed` separately) a script may set.
pub const MAX_VALUE_BYTES: usize = 1_048_576;
/// The most the scripts of a job may add to its variables: keys, `text` and `typed` of all four lists, over
/// what the job came with.
pub const MAX_VARIABLES_BYTES: usize = 16 * 1_048_576;
/// The most entries and bytes (keys and values) of `Report::changes`.
pub const MAX_CHANGES: usize = 10_000;
pub const MAX_CHANGES_BYTES: usize = 16 * 1_048_576;

#[derive(Debug, Clone, Default)] pub struct Info { pub request_id: String, pub request_name: String }

/// One variable a script sees. `typed` — JSON of the value a script of this send has set (a number, an object…).
#[derive(Debug, Clone, PartialEq)]
pub struct Var { pub key: String, pub text: String, pub typed: Option<String> }
impl Var { pub fn new(key: impl Into<String>, text: impl Into<String>) -> Self; }   // typed: None

/// The variables of a send: enabled ones only, in their stored order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Variables { pub globals: Vec<Var>, pub collection: Vec<Var>, pub environment: Vec<Var>,
                       /// `None` — no environment is chosen: `pm.environment` works but nothing is stored.
                       pub environment_name: Option<String>,
                       pub local: Vec<Var> }
impl Variables { /// Global < Collection < Environment < Local, by `text`.
                 pub fn resolver(&self) -> rqm_vars::Resolver; }

#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum StoredScope { Globals, Collection, Environment }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableOp { Set { key: String, value: String }, Unset { key: String }, Clear }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct VariableChange { pub scope: StoredScope, pub op: VariableOp }

#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum ConsoleLevel { Log, Info, Warn, Error, Debug }
pub struct ConsoleLine { /// `None` — a line of the application itself.
                         pub script: Option<usize>, pub level: ConsoleLevel, pub text: String,
                         /// Whole milliseconds since `Job::epoch`.
                         pub at_ms: u64 }
// Job:    + pub info: Info, pub variables: Variables
// Report: + pub console: Vec<ConsoleLine>, pub console_truncated: bool,
//           pub variables: Variables,            // after the last script that ran
//           pub changes: Vec<VariableChange>     // in the order the scripts made them
```

**Сборка прелюдии:**
- `prelude/package.json`: `"private": true`, `"type": "module"`, `"scripts": { "build": "node build.mjs" }`,
  `devDependencies: { "esbuild": "0.28.2" }`.
- `build.mjs`: esbuild `{ bundle: true, format: "iife", platform: "neutral", mainFields: ["main", "module"],
  target: "es2020", minify: true, legalComments: "none" }`, вход `src/index.js` → `built/prelude.js`. Первая
  строка каждого собранного файла — `// rqm-prelude-hash: <16 шестнадцатеричных цифр>`.
- Контрольная сумма — FNV-1a 64 бит (начало `0xcbf29ce484222325`, множитель `0x100000001b3`) по
  последовательности: для каждого файла из списка `build.mjs`, `package.json`, `package-lock.json`, затем все
  файлы `prelude/src/` по имени — имя файла относительно `prelude/` с `/`, байт `0x0A`, содержимое **без байтов
  `0x0D`**, байт `0x0A`. Одно и то же считают `build.mjs` (BigInt) и тест Rust — так тест без Node ловит «исходник
  поменяли, а пересобрать забыли», а перевод строк Windows сумму не меняет.
- `src/bundle.rs`: `pub(crate) enum Bundle { Prelude }`, `pub(crate) fn load(ctx: &Ctx<'_>, bundle: Bundle) ->
  rquickjs::Result<()>`. Исходник — `include_str!`. Байткод — в `static OnceLock<Vec<u8>>` на bundle: при первой
  надобности `Module::declare(…).write(WriteOptions { strip_source: true, strip_debug: true, ..Default::default()
  })`; дальше в каждый контекст `unsafe { Module::load(ctx.clone(), bytes) }` → `eval`. Promise, который вернул
  `eval`, уже `Resolved` (в bundle нет `await` верхнего уровня — проверено опытом), и очередь заданий здесь не
  крутится: bundle может грузиться посреди скрипта (Task 5), и микрозадачи скрипта исполнились бы раньше
  времени. Состояние не `Resolved` — ошибка загрузки. Над `unsafe` — комментарий с инвариантом: байты сделаны
  этим же движком в этом же процессе и с диска не читаются. На время загрузки bundle секундомер скрипта стоит.

**Правила:**
- Мост: перед загрузкой прелюдии Rust кладёт в глобальную область объект `__host` с функциями-примитивами
  (каждая с именем `rqm_host_<имя>`); прелюдия первым делом забирает его в замыкание и удаляет (`delete
  globalThis.__host`). Через границу ходят строки, числа, булевы; структуры — текстом JSON. Примитивы не
  паникуют: на негодный аргумент — исключение JS. Действуют правила движка № 6–9: предел проверяет примитив, а не
  прелюдия; примитив первым делом проверяет флаг `abandoned` (Task 1); замок состояния — `try_lock`, без вызовов
  в JS под замком.
- Строки на границе: в прелюдии одна обёртка `wellFormed(s)` — `String.prototype.toWellFormed`, сохранённый в
  замыкание при загрузке и вызываемый через сохранённый же `Reflect.apply` (скрипт может подменить прототип).
  Через неё проходит **каждая** строка, уходящая в примитив: текст консоли, ключ и значение `set / unset / has /
  get`, аргумент `replaceIn`, а в следующих задачах — имя и ошибка теста, строки `pm.request.*`,
  `pm.sendRequest`, `postman.set*`. `typed` (результат `JSON.stringify`) в обёртке не нуждается, но проходит
  через неё же.
- Порядок на скрипт: `__host` → прелюдия → обёрнутый текст скрипта.
- `console.log / info / warn / error / debug`: аргументы приводит к тексту прелюдия — строка как есть; `undefined`
  → `undefined`; функция → `[Function: имя]`; объект с текстовыми `name` и `message` → `name: message`; BigInt —
  число и `n`; остальное — `JSON.stringify`, цикл заменяется на `"[Circular]"`; не получилось — `String(value)`.
  Аргументы через пробел. Обрезка — в два шага: прелюдия грубо режет строку до `2 * CONSOLE_LINE_CHARS` единиц
  UTF-16 (чтобы через границу не шла строка любой длины) и **после этого** зовёт `wellFormed`; точную обрезку до
  `CONSOLE_LINE_CHARS` символов и `…` в конце делает примитив (`text::clip`). `console.clear()` ничего не
  делает. Других методов у `console` нет.
- Предел консоли — на весь `Job`, считает примитив: после `CONSOLE_MAX_LINES` строк или `CONSOLE_MAX_BYTES` байт
  текста новые строки отбрасываются, `console_truncated = true`.
- Пределы переменных считает Rust (`src/variables.rs`), до копирования значения из JS (длина —
  `to_cstring()?.len()`, правило движка № 8):
  - `text` или `typed` значения длиннее `MAX_VALUE_BYTES` → `set` бросает обычное исключение, которое скрипт
    может поймать: `Error: pm.<область>.set: the value of '<ключ>' is larger than 1 MB` (`<область>` —
    `globals`, `collectionVariables`, `environment`, `variables`). Значение не меняется, в журнал не идёт. Ключ
    длиннее `MAX_VALUE_BYTES` — та же ошибка с текстом `the key is larger than 1 MB`.
  - После операции суммарный объём `Variables` (ключи, `text`, `typed` четырёх списков) превысил бы объём на
    входе `Job` больше чем на `MAX_VARIABLES_BYTES` (большое окружение пользователя само пределом не считается),
    либо журнал — длиннее `MAX_CHANGES` записей или `MAX_CHANGES_BYTES` байт → операция
    не выполняется, выставляется флаг предела памяти хоста. По этому флагу обработчик прерывания останавливает
    скрипт (`try/catch` скрипта это не ловит), а любой следующий примитив отказывает сразу. Вид ошибки —
    `Memory`, текст: `the script stored more than 16 MiB of variables` (объём переменных), `the script changed
    variables more than 10000 times` (число записей журнала) либо `the script wrote more than 16 MiB of variable
    values` (байты журнала). В порядке флагов Task 1: `Cancelled` → `Timeout` →
    `Memory`. Изменения, сделанные до превышения, остаются в отчёте, как при отмене.
  - Склейки журнала (несколько `set` одного ключа в одну запись) нет: порядок записей — то, что проверяют
    тесты и чем пользуется запись в хранилище.
- Области: `pm.globals`, `pm.collectionVariables`, `pm.environment` — каждая над своим списком;
  `pm.variables` читает local → environment → collection → globals, пишет только в local.
  - `get(key)` — значение последней записи с таким ключом, иначе `undefined`; у записи с `typed` — разобранный
    JSON, иначе `text`. `has(key)`. Ключ приводится `String(key)`.
  - `set(key, value)` — меняет последнюю запись с ключом, иначе добавляет в конец. `text`: `undefined` → `""`,
    иначе `String(value)`; `typed`: `JSON.stringify(value)`, когда значение не строка и результат — строка, иначе
    `None`. Третий аргумент не используется.
  - `unset(key)` — удаляет все записи с ключом; `clear()` — все записи области.
  - `toObject()` — `{ ключ: значение }` (у `pm.variables` — слияние от широкой области к узкой).
  - `replaceIn(x)` — строка: подстановка `Resolver::substitute`; у `pm.variables` — по `Variables::resolver()`,
    у остальных — только по своей области; объект и массив — рекурсивно по строковым значениям (новый объект);
    прочее возвращается как есть.
  - `pm.environment.name` — `environment_name` или `undefined`.
- Журнал: `set` / `unset` / `clear` хранимой области добавляют `VariableChange`; `unset` несуществующего ключа и
  `clear` пустой области — нет. При `environment_name == None` изменения `pm.environment` в журнал не идут, а
  первый такой `set` в скрипте добавляет строку консоли `Warn`: `pm.environment.set: no environment is selected,
  the value is not saved`. `pm.variables` в журнал не пишет никогда.
- `pm.info`: `eventName` — `prerequest` / `test`; `requestName`, `requestId` из `Info`; `iteration` — `0`;
  `iterationCount` — `1`.

- [ ] **Step 1: Падающие тесты.**
  `tests/bundle.rs`: `built_prelude_matches_its_sources` (пересчитать сумму по правилу выше, сравнить с первой
  строкой `built/prelude.js`); `prelude_loads_into_every_script` (30 скриптов `console.log(pm.info.eventName)` в
  одном `Job` → 30 строк `test`); `host_primitives_are_out_of_reach` (`"if (typeof __host !== 'undefined') throw
  new Error('host')"` → `errors` пуст).
  Модульные тесты примитивов (`#[cfg(test)]` в `src/host.rs` и `src/variables.rs` — примитивы вызываются как
  функции Rust над состоянием, без JS и без «тестового режима» песочницы): `console_primitive_enforces_its_limits`
  — строка из 20 000 символов → 10 000 и `…`; 5000 вызовов → 1000 строк, `console_truncated`; строки общим
  объёмом больше `CONSOLE_MAX_BYTES` → отброшены; `variable_primitives_enforce_their_limits` — значение из
  `MAX_VALUE_BYTES + 1` байт → ошибка, список и журнал прежние; 10 001-й `set` → флаг памяти хоста, в журнале
  ровно `MAX_CHANGES`; прирост больше `MAX_VARIABLES_BYTES` → флаг; `primitives_refuse_when_abandoned` — при
  выставленном `abandoned` каждый примитив отвечает ошибкой и ничего не меняет; `busy_state_is_an_error_not_a_
  panic` — замок состояния занят → примитив отвечает ошибкой.
  `tests/tamper.rs` (расширяется в Task 3, 5, 6, 8 — каждый раз на API этой задачи):
  `tampered_builtins_do_not_lift_limits` — скрипт сначала подменяет `String.prototype.slice / substring /
  toWellFormed`, `Array.prototype.push / map / forEach / join`, `JSON.stringify`, `Function.prototype.call /
  apply / bind`, `Reflect.apply`, `Promise.prototype.then` (подмены возвращают «ничего не обрезано / всё
  разрешено»), затем `for (let i = 0; i < 5000; i++) console.log('x'.repeat(20000)); for (;;)
  pm.environment.set('i', 1)` → в отчёте ≤ 1000 строк, каждая ≤ 10 001 символа, `changes.len() ≤ MAX_CHANGES`,
  ошибка `Memory`; `host_primitives_do_not_leak` — подменённые `Function.prototype.call / apply / bind`,
  `Reflect.apply` и методы массива складывают в список каждое встреченное `this` и каждый аргумент-функцию;
  скрипт вызывает весь API задачи (`console.*`, все методы четырёх областей, `pm.info`) и печатает, сколько
  собранных функций имеют `name`, начинающийся с `rqm_host_` → `0`.
  `tests/console.rs`: `console_levels_and_formatting` — скрипт `console.log('a', 1, true, null, undefined, { b:
  [1, { c: 'я' }] }); console.info(new TypeError('t')); console.warn(function named() {}); console.error('e');
  console.debug('d'); const o = {}; o.self = o; console.log(o); console.clear();` → шесть строк: `Log` `a 1 true
  null undefined {"b":[1,{"c":"я"}]}`, `Info` `TypeError: t`, `Warn` `[Function: named]`, `Error` `e`, `Debug`
  `d`, `Log` `{"self":"[Circular]"}`; у всех `script == Some(0)`, `at_ms` не убывает; `console_is_capped` — `for
  (let i = 0; i < 5000; i++) console.log(i)` → 1000 строк, `console_truncated`; `console.log('x'.repeat(20000))` →
  10 001 символ, последний `…`; три скрипта по 120 строк из 5000 символов → сумма длин ≤ `CONSOLE_MAX_BYTES`,
  `console_truncated`; `endless_logging_ends_by_the_time_limit` — `while (true) console.log(1)`, 200 мс →
  `Timeout`, 1000 строк; `console_table_is_absent_like_in_postman` — `console.table([1])` → `Exception`,
  начинается с `TypeError`; `non_ascii_and_lone_surrogates_in_the_console` — `console.log('я'.repeat(20000))` →
  10 001 символ (`chars().count()`), последний `…`; `console.log('😀'.repeat(6000))` → одна строка из 6000
  символов, `errors` пуст; `console.log('a' + '😀'.repeat(20000))` → 10 001 символ, предпоследний — целый
  эмодзи; `console.log(String.fromCharCode(0xd800))` → текст `\u{FFFD}`; `console.log(JSON.parse('"\\ud83d"'))`
  → то же; `console.log('x😀'.slice(0, 2))` → `x\u{FFFD}`; во всех случаях `errors` пуст.
  `tests/variables.rs` (исходно: globals `g=G, shared=g`; collection `c=C, shared=c`; environment `e=E, shared=e`,
  имя `Dev`; `Info { request_id: "r-1", request_name: "Login" }`):
  `narrowest_scope_wins` — `console.log(pm.variables.get('shared'), pm.globals.get('shared'),
  pm.collectionVariables.get('shared'), pm.environment.get('shared'), pm.variables.get('nope'),
  pm.variables.has('g'), pm.environment.has('g'))` → `e g c e undefined true false`; после `pm.variables.set(
  'shared', 'l')` — `pm.variables.get('shared')` даёт `l`, а `pm.environment.get('shared')` — `e`;
  `set_updates_the_last_entry_or_appends_and_is_logged` — environment `a=1, a=2`; `pm.environment.set('a', 'x');
  pm.environment.set('b', 'y'); pm.globals.set('g2', 'z'); pm.collectionVariables.set('token', 't');
  pm.variables.set('loc', 'v'); pm.globals.set(7, 'k', 'string')` → `report.variables.environment` — `a=1, a=x,
  b=y`; `changes` — `[Environment Set a x, Environment Set b y, Globals Set g2 z, Collection Set token t, Globals
  Set "7" k]`; `local` — `loc=v`;
  `values_keep_their_type_within_a_send_and_are_stored_as_text` — скрипт 1: `pm.environment.set('n', 5);
  pm.environment.set('o', { a: [1] }); pm.environment.set('z', null); pm.environment.set('u', undefined);
  pm.environment.set('b', true)`; скрипт 2: `console.log(typeof pm.environment.get('n'), pm.environment.get(
  'o').a[0], pm.environment.get('z') === null, pm.environment.get('u') === '', pm.environment.get('b') === true)`
  → `number 1 true true true`; значения в `changes`: `5`, `[object Object]`, `null`, пустая строка, `true`;
  `unset_and_clear` — `pm.environment.unset('e'); pm.environment.unset('nope'); pm.globals.clear();
  pm.collectionVariables.clear(); pm.collectionVariables.clear()` → `changes` — `[Environment Unset e, Globals
  Clear, Collection Clear]`;
  `to_object_and_replace_in` — `JSON.stringify(pm.variables.toObject())` → `{"g":"G","shared":"e","c":"C","e":
  "E"}`; `pm.variables.replaceIn('{{g}}-{{c}}-{{e}}-{{nope}}')` → `G-C-E-{{nope}}`; `pm.environment.replaceIn(
  '{{g}}{{e}}')` → `{{g}}E`; `JSON.stringify(pm.variables.replaceIn({ a: ['{{g}}', 1] }))` → `{"a":["G",1]}`;
  `pm.variables.replaceIn('{{$guid}}')` — 36 символов вида UUID;
  `environment_set_without_an_environment_lives_for_the_send_only` — `environment_name: None`; скрипт 1
  `pm.environment.set('t', '1'); pm.environment.set('t2', '2')`, скрипт 2 `console.log(pm.environment.get('t'),
  pm.environment.name)` → `1 undefined`; `changes` пуст; в консоли ровно одна строка `Warn` с текстом из правил;
  `info_of_the_run` — `PreRequest`: `console.log(pm.info.eventName, pm.info.requestName, pm.info.requestId,
  pm.info.iteration, pm.info.iterationCount)` → `prerequest Login r-1 0 1`;
  `changes_made_before_an_error_are_kept` — `pm.globals.set('a', '1'); throw new Error('x')` → в `changes` есть
  `Set a`, в `errors` — ошибка;
  `endless_set_is_a_memory_error` — предел времени 10 с, `try { while (true) pm.globals.set('k', 'x'.repeat(1 <<
  19)) } catch (e) {}` → `Memory` быстрее 2 с, текст `the script wrote more than 16 MiB of variable values` (ключ один —
  растёт журнал, а не список); `changes.len() ≤ MAX_CHANGES`, сумма длин значений ≤
  `MAX_CHANGES_BYTES`; следующий `Job` работает;
  `endless_small_sets_hit_the_change_count` — `for (;;) pm.environment.set('i', 1)` → `Memory`, в `changes`
  ровно `MAX_CHANGES` записей;
  `local_variables_are_capped` — `for (let i = 0; ; i++) pm.variables.set('k' + i, 'x'.repeat(1 << 19))` →
  `Memory` (журнал при этом пуст — `pm.variables` в него не пишет);
  `too_long_value_is_a_catchable_error` — `try { pm.environment.set('e', 'x'.repeat((1 << 20) + 1)) } catch (err)
  { console.log(err.message) } console.log(pm.environment.get('e'))` → `pm.environment.set: the value of 'e' is
  larger than 1 MB`, затем `E`; `changes` пуст, `errors` пуст; объект, чей `JSON.stringify` длиннее предела, —
  то же; значение ровно из `1 << 20` байт принимается;
  `big_environment_of_the_user_is_not_the_limit` — на входе окружение с одной переменной в 20 МиБ; скрипт
  `pm.environment.set('a', '1')` → `errors` пуст;
  `lone_surrogates_in_keys_and_values` — `pm.environment.set('k', 'a\ud800b'); pm.globals.set('\ud83d', 'v')` →
  `changes == [Environment Set k "a\u{FFFD}b", Globals Set "\u{FFFD}" v]`, `errors` пуст;
  `changes_made_before_the_watchdog_are_reported` — `Limits { time: 100 мс, grace: 100 мс, .. }`,
  `pm.globals.set('a', '1'); (10n ** 300000n).toString(); pm.globals.set('late', '1')` → `Timeout` быстрее
  1,5 с; в `changes` есть `Set a` и нет `late`; тест ждёт, пока `abandoned_threads()` не вернётся к `0`.
- [ ] **Step 2: Запустить** `cargo test -p rqm-script` — новые тесты не собираются или падают.
- [ ] **Step 3: Реализовать.** В `crates/rqm-script/prelude/`: `npm install`, `npm run build`; `built/prelude.js`
  и `package-lock.json` — в коммит (`git status` должен показать `built/` как неотслеживаемый, а не игнорируемый).
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: прелюдия pm.* — сборка, console, переменные четырёх областей, pm.info».

---

### Task 3: `pm.response`, `pm.test`, старые имена ответа

**Files:**
- Create: `crates/rqm-script/prelude/src/lists.js`, `response.js`, `test.js`, `legacy.js`
- Modify: `prelude/src/index.js`, `prelude/built/prelude.js`, `crates/rqm-script/src/lib.rs`, `src/host.rs`,
  `src/engine.rs`
- Test: `crates/rqm-script/tests/response.rs`, `tests/tests_api.rs`, `tests/tamper.rs`; модульные тесты примитива
  записи теста в `src/host.rs`

**Interfaces:**
- Consumes: Task 2 (мост `__host`, обёртка `wellFormed`, журнал, консоль); `text::clip` (Task 1).
- Produces:

```rust
/// The most of a response body a script can get: 10 MiB.
pub const BODY_LIMIT: u64 = 10 * 1024 * 1024;
/// The most tests one job records.
pub const MAX_TESTS: usize = 1000;
/// The longest name and error text of a test, in characters.
pub const TEST_NAME_CHARS: usize = 1000;
pub const TEST_ERROR_CHARS: usize = 10_000;
/// The most text (UTF-8 bytes of the names and the errors) the tests of one job hold.
pub const MAX_TEST_TEXT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct ScriptResponse { pub code: u16, /// Reason phrase.
                            pub status: String, pub headers: Vec<(String, String)>,
                            pub time_ms: f64, /// Size of the decoded body in bytes.
                            pub size: u64, pub body: ResponseBody }
#[derive(Debug, Clone)]
pub enum ResponseBody { /// Read from the file when a script asks for the body; the file is not deleted.
                        File(PathBuf), Bytes(Vec<u8>) }

pub struct TestResult { pub script: usize, pub name: String, pub passed: bool, pub skipped: bool,
                        pub error: Option<String> }
// Job:    + pub response: Option<ScriptResponse>     // Some only in Phase::Test
// Report: + pub tests: Vec<TestResult>
```

**Правила:**
- `pm.response` и старые имена ответа есть, только когда `job.response` задан; иначе `pm.response`,
  `responseBody`, `responseCode`, `responseHeaders`, `responseTime` — `undefined`.
- `pm.response`: `code`, `status`, `reason()` (то же, что `status`), `responseTime` (`time_ms`), `responseSize`
  (`size`), `headers`, `text()`, `json()`.
- Тело читается при первом обращении к `text()`, `json()` или `responseBody` и запоминается на скрипт: не больше
  `BODY_LIMIT + 1` байт из файла (или переданные байты); на время чтения секундомер стоит. Больше `BODY_LIMIT` →
  `text()` и `json()` бросают `Error` с текстом `response body is larger than 10 MB and is not available to
  scripts`, а `responseBody` равен `undefined`. Файл не читается → `Error: response body cannot be read: <причина>`.
  Текст — UTF-8 с заменой негодных байтов на U+FFFD, BOM в начале отбрасывается.
- `json()` — `JSON.parse(text())`; при неудаче бросает ошибку с `name = "JSONError"` и сообщением разбора.
- `lists.js` — один класс списка пар для заголовков ответа (а в Task 8 — заголовков и параметров запроса):
  `get(name)` — значение **последней** пары с именем, без учёта регистра; `has(name)`; `all()` — массив `{ key,
  value }`; `each(fn)`; `count()`; `toObject()` — ключи в нижнем регистре, повторяющееся имя — массив значений.
- Старые имена — ленивые свойства глобального объекта: `responseBody` — `pm.response.text()`; `responseCode` — `{
  code, name: status, detail: status }`; `responseHeaders` — объект «имя как пришло → значение», при повторе —
  последнее; `responseTime` — `time_ms`. Ленивое свойство — пара «геттер и **сеттер**», `configurable: true`:
  геттер считает значение при первом чтении; сеттер заменяет свойство обычным записываемым значением. Так его
  затеняет и `var responseBody = …` скрипта, и присваивание без объявления (`responseBody = 'x'`: скрипт
  исполняется нестрого, и у свойства с одним геттером такое присваивание молча ничего бы не делало). Это же
  правило — у всех ленивых глобальных свойств следующих задач (`tests`, `environment`, `globals`, `request`,
  `data`, `iteration`, `_`).
- `pm.test(name, fn)`: `fn` не функция → тест пройден; исключение в `fn` → провален, `error` — `"{name}:
  {message}"` для объекта с `message` (без `name` — `Error`), иначе строковая форма брошенного; скрипт идёт
  дальше. Возвращает `pm`. `pm.test.skip(name)` → `{ passed: true, skipped: true }`, функция не вызывается.
  `pm.test.index()` — сколько тестов этот скрипт уже записал. Имя — `String(name)`; имя и `error` проходят
  `wellFormed` (Task 2). Пределы считает примитив записи теста (правило движка № 6): имя обрезается до
  `TEST_NAME_CHARS` символов, `error` — до `TEST_ERROR_CHARS` (`text::clip`; прелюдия перед этим грубо режет до
  двойной длины в единицах UTF-16, как у консоли). Тест сверх `MAX_TESTS` в одном `Job` → примитив отказывает, и
  `pm.test` бросает `Error: too many tests: more than 1000`; сумма байт UTF-8 имён и ошибок тестов `Job` вместе с
  этим тестом превысила бы `MAX_TEST_TEXT_BYTES` → `Error: too many tests: their results are larger than 256
  KB`. Обе — ошибки скрипта, а не теста; уже записанные тесты остаются.

- [ ] **Step 1: Падающие тесты.**
  `tests/response.rs` (ответ по умолчанию: 200, `OK`, заголовки `Content-Type: application/json`, `X-A: 1`,
  `x-a: 2`, время 12.5, тело байтами):
  `response_fields` — тело `{"token":"abc","n":[1,2]}`: `console.log(pm.response.code, pm.response.status,
  pm.response.reason(), pm.response.responseTime, pm.response.responseSize, pm.response.headers.get(
  'content-type'), pm.response.headers.get('X-A'), pm.response.headers.has('nope'), pm.response.headers.count(),
  pm.response.json().n[1], pm.response.text() === '{"token":"abc","n":[1,2]}')` → `200 OK OK 12.5 25
  application/json 2 false 3 2 true` (при `size: 25`);
  `headers_to_object_lowercases_and_groups` — `JSON.stringify(pm.response.headers.toObject())` →
  `{"content-type":"application/json","x-a":["1","2"]}`; `JSON.stringify(pm.response.headers.all()[1])` →
  `{"key":"X-A","value":"1"}`;
  `owner_scenario_parses_the_body_and_sets_a_collection_variable` — тело `{"access_token":"T-1"}`, скрипт `var
  data = JSON.parse(responseBody);\npm.collectionVariables.set("token", data.access_token);` → `errors` пуст,
  `changes == [Collection Set token T-1]`;
  `legacy_response_names` — `console.log(responseCode.code, responseCode.name, responseHeaders['Content-Type'],
  responseHeaders['x-a'], responseTime)` → `200 OK application/json 2 12.5`;
  `bom_bad_utf8_and_empty_bodies` — байты `EF BB BF` + `{"a":1}` → `pm.response.json().a === 1`; байты `FF 41` →
  `pm.response.text() === '�A'`; пустое тело → `pm.response.text() === ''`, а `pm.test('j', () =>
  pm.response.json())` провален с `error`, начинающимся на `JSONError`;
  `body_file_is_read_only_when_asked` — `ResponseBody::File(<несуществующий путь>)`: `console.log(
  pm.response.code)` → `errors` пуст; `pm.response.text()` → `Exception`, начинается с `Error: response body
  cannot be read`;
  `body_over_the_limit_is_refused` — файл из `BODY_LIMIT + 1` байт: `pm.test('t', () => { pm.response.json() });
  console.log(typeof responseBody)` → тест провален с `Error: response body is larger than 10 MB and is not
  available to scripts`, в консоли `undefined`; файл ровно из `BODY_LIMIT` байт `a` → `pm.response.text().length
  === 10485760`;
  `no_response_before_the_request` — `PreRequest`: `console.log(typeof pm.response, typeof responseBody, typeof
  responseCode, typeof responseHeaders, typeof responseTime)` → пять раз `undefined`;
  `legacy_names_can_be_assigned_without_var` — `responseBody = 'x'; responseTime = 1; console.log(responseBody,
  responseTime); var responseCode = 2; console.log(responseCode)` → `x 1`, затем `2`.
  `tests/tests_api.rs`: `tests_pass_fail_skip_and_do_not_stop_the_script` — `pm.test('ok', function () {});
  pm.test('bad', function () { throw new Error('boom') }); pm.test('str', () => { throw 'plain' }); pm.test('no
  function'); pm.test.skip('later', () => { throw new Error('never') }); console.log(pm.test.index(),
  pm.test('chain', () => {}) === pm)` → `tests`: `ok` пройден; `bad` провален, `Error: boom`; `str` провален,
  `plain`; `no function` пройден; `later` — `skipped`, `passed`; `chain` пройден; консоль `5 true`; `errors` пуст;
  `tests_name_their_script` — два скрипта с тестом `same` → `script` 0 и 1;
  `too_many_tests_is_an_error_of_the_script` — `for (let i = 0; i < 1001; i++) pm.test('t' + i, () => {})` → 1000
  тестов и одна ошибка `Exception` с `too many tests`;
  `names_and_errors_are_capped` — имя из 5000 символов → 1000; сообщение из 20 000 → 10 000 (длины —
  `chars().count()`); имена `'я'.repeat(5000)` и `'😀'.repeat(5000)` → по 1000 символов, последний — целый
  символ; сообщение `'😀'.repeat(20000)` → 10 000 символов; `errors` пуст;
  `lone_surrogates_in_tests` — `pm.test('\ud83d', () => {}); pm.test('t', () => { throw new Error('\ud800') })` →
  два теста: с именем `\u{FFFD}` — пройден; `t` — провален, `Error: \u{FFFD}`; `errors` пуст;
  `test_text_is_capped_as_a_whole` — `for (let i = 0; i < 1000; i++) pm.test('t' + i, () => { throw new
  Error('я'.repeat(10000)) })` → сумма байт имён и ошибок в `tests` ≤ `MAX_TEST_TEXT_BYTES`, тестов меньше 1000,
  одна ошибка `Exception` с текстом `their results are larger than 256 KB`;
  `tests_before_an_error_are_kept` — `pm.test('a', () => {}); throw new Error('x')` → один тест и одна ошибка.
  Модульный тест `test_primitive_enforces_its_limits` (`src/host.rs`): имя из 5000 символов → 1000; 1001-й тест →
  отказ; тест, с которым сумма текста превысила бы `MAX_TEST_TEXT_BYTES`, → отказ, прежние на месте.
  `tests/tamper.rs`: оба теста Task 2 дополнить `pm.test` (1001 тест с именем из 5000 символов при подменённых
  встроенных объектах → ≤ 1000 тестов, имена ≤ 1000 символов) и вызовами `pm.response.*`, `pm.test.*`.
- [ ] **Step 2: Запустить** `cargo test -p rqm-script` — новые тесты падают.
- [ ] **Step 3: Реализовать**; `npm run build` в `prelude/`.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: pm.response, pm.test и старые имена ответа».

---

### Task 4: Конвейер отправки со скриптами (первая веха)

После этой задачи в тестах ядра работает сквозной сценарий: pre-request ставит переменную, запрос уходит с ней,
test-скрипт проверяет ответ через `pm.test` и кладёт значение в переменную коллекции. Приложение ведёт себя как
раньше: `load_send_input` до Task 10 объявляет скрипты выключенными.

**Files:**
- Modify: `crates/rqm-runner/Cargo.toml` (+ `rqm-script = { path = "../rqm-script" }`, + `tokio = { workspace =
  true, features = ["rt", "sync"] }`), `src/lib.rs`, `src/prepare.rs`, `src/send.rs`, `src/scope.rs` (комментарии)
- Create: `crates/rqm-runner/src/scripts.rs`
- Modify (только чтобы workspace собирался): `src-tauri/src/send.rs`
- Test: `crates/rqm-runner/tests/scripts.rs` (новый), `tests/prepare.rs`, `tests/send.rs`, `tests/scope.rs`,
  `tests/owner_end_to_end.rs`

**Interfaces:**
- Consumes: `rqm_script::{run, Job, Phase, Report, Info, Variables, Var, ScriptResponse, ResponseBody,
  ScriptError, ScriptErrorKind, TestResult, ConsoleLine, ConsoleLevel, VariableChange, VariableOp, StoredScope}`
  (Task 1–3); `ScopeSources::of_input`; `rqm_http::HttpResponse { status, status_text, headers, body_path,
  decoded_size, timings.total, .. }`.
- Produces:

```rust
// crates/rqm-runner/src/lib.rs
#[derive(Debug, Clone, PartialEq, Eq)] pub enum ScriptsMode { Run, Off(ScriptsOff) }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptsOff { /// Switched off in the settings.
                      Disabled,
                      /// The collection has no consent; holds its name.
                      NotAllowed { collection: String } }
pub struct SendInput { /// Id of the request node: `pm.info.requestId`, the source of its scripts.
                       pub request_id: String,
                       /* request_name, request, ancestors, environment, globals, settings — как раньше */
                       /// Whether the scripts of this send run.
                       pub scripts: ScriptsMode }
// load_send_input: подпись прежняя; request_id — как передан; scripts — ScriptsMode::Off(ScriptsOff::Disabled)

// crates/rqm-runner/src/scripts.rs, re-exported from lib.rs; все типы — #[derive(Debug, Clone, PartialEq)]
pub use rqm_script::{ConsoleLevel, ScriptErrorKind, StoredScope, VariableChange, VariableOp};
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum ScriptPhase { PreRequest, Test }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptSource { pub node_id: String, pub node_name: String, pub node_kind: NodeKind, pub phase: ScriptPhase }
pub struct ScriptStep { pub source: ScriptSource, pub code: String }
/// The scripts of the phase that are not blank: the collection, the folders from the outer one, the request.
pub fn script_steps(input: &SendInput, phase: ScriptPhase) -> Vec<ScriptStep>;
#[derive(Debug, Clone, PartialEq)]
pub struct TestOutcome { pub source: ScriptSource, pub name: String, pub passed: bool, pub skipped: bool, pub error: Option<String> }
pub struct ConsoleEntry { pub source: Option<ScriptSource>, pub level: ConsoleLevel, pub text: String, pub at_ms: u64 }
pub struct ScriptFailure { pub source: ScriptSource, pub kind: ScriptErrorKind, pub message: String,
                           pub line: Option<u32>, pub column: Option<u32> }
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScriptsReport { pub tests: Vec<TestOutcome>, pub console: Vec<ConsoleEntry>, pub console_truncated: bool,
                           pub errors: Vec<ScriptFailure>, pub changes: Vec<VariableChange> }

// crates/rqm-runner/src/send.rs
pub enum SendError { Prepare(PrepareError), Http(HttpError),
                     /// A pre-request script failed; nothing was sent.
                     #[error("script error: {}", .0.message)] Script(ScriptFailure),
                     /// The send was cancelled while a pre-request script ran.
                     #[error("cancelled")] Cancelled }
pub struct SendOutcome { /* result, warnings, request — как раньше */ pub scripts: ScriptsReport }

// crates/rqm-runner/src/prepare.rs
pub enum SendWarning { /// Scripts exist but did not run; names of the nodes that have one, from the collection down.
                       ScriptsNotRun { reason: ScriptsOff, nodes: Vec<String> },
                       UnsupportedAuth { .. }, BodyPruned { .. }, UnresolvedVariables { .. } }
/// `prepare` with the resolver given by the caller.
pub(crate) fn prepare_with(input: &SendInput, resolver: Resolver,
                           dynamic: &mut dyn FnMut(&str) -> Option<String>) -> Result<Prepared, PrepareError>;
// prepare(input, dynamic) = prepare_with(input, ScopeSources::of_input(input).resolver(), dynamic);
// предупреждения о скриптах prepare больше не даёт — это дело send.
```

**Правила `send`:**
1. Неподтверждённые файлы — отказ `Prepare(UnconfirmedFiles)` до любого скрипта (как сейчас; проверка
   `unconfirmed_file_fields` вызывается первой).
2. `pre = script_steps(input, PreRequest)`, `tests = script_steps(input, Test)`. Скрипт пуст, когда его текст пуст
   после `trim` — по каждой фазе отдельно. Источник запроса: `node_id = input.request_id`, `node_name =
   input.request_name`, `node_kind = Request`.
3. `input.scripts == Off(reason)`: ничего не исполняется; если хоть один шаг есть — первым предупреждением
   `ScriptsNotRun { reason, nodes }` (имена узлов со скриптом любой фазы, от коллекции к запросу, без повторов);
   дальше как на этапе 1.
4. `Run` и шагов нет — как на этапе 1, поток не создаётся.
5. Иначе: `Variables` — из `ScopeSources::of_input(input)` (только включённые, `Var::new(key, value)`),
   `environment_name` — имя выбранного окружения; `Info` — `request_id`, `request_name`; `epoch` — момент начала
   `send`, один на обе фазы.
6. Фаза pre-request (если `pre` не пуст): `rqm_script::run` в `tokio::task::spawn_blocking`. Отчёт переводится в
   `ScriptsReport` (индекс скрипта → `ScriptSource` шага). Есть ошибка: вид `Cancelled` → `Err(SendError::
   Cancelled)`, иначе `Err(SendError::Script(первая ошибка))`; `request: None`, `warnings` пуст; test-скрипты не
   исполняются. Задача `spawn_blocking` завершилась ошибкой (`JoinError` — паника) → `ScriptFailure { kind:
   Internal, message: "script thread failed" }` первого шага; ветка достижима только в dev- и test-профиле (в
   release `panic = "abort"`, правило движка № 3). Скрипт, брошенный сторожем (Task 1), приходит обычной ошибкой
   `Timeout` или `Cancelled` — отдельной обработки нет; `run` возвращается не позже «предел + запас» на скрипт.
7. `prepare_with(input, variables.resolver(), dynamic)` — с переменными после pre-request, включая local.
8. `engine.send`. Ответа нет — test-скрипты не исполняются.
9. Фаза test (ответ есть и `tests` не пуст): `ScriptResponse { code: status, status: status_text, headers,
   time_ms: timings.total в мс, size: decoded_size, body: ResponseBody::File(body_path) }`. Ошибки test-скриптов
   результат отправки не меняют.
10. `SendOutcome.scripts` — тесты, консоль, ошибки и изменения обеих фаз по порядку.

**Прочее:**
- `scope.rs`: комментарий у `resolver()` и у ветки `ScopeKind::Local` в `variables_in_scope` — «local живёт одну
  отправку и в подсветку не попадает; отправка получает его через `prepare_with`». Комментарии `lib.rs`
  («"Send" scenario without scripts», `request_name` — «used to name the node in ScriptsNotExecuted») и
  `Prepared.warnings` («In the order: scripts, …») привести в соответствие.
- `src-tauri/src/send.rs` — временно, до Task 11: `warning_dto`: `ScriptsNotRun { nodes, .. }` →
  `(WarningKind::ScriptsNotExecuted, nodes)`; `error_dto`: `SendError::Script(_)` → `SendErrorKind::Other`,
  `SendError::Cancelled` → `SendErrorKind::Cancelled`. Тесты оболочки не меняются и остаются зелёными.
- Изменённые утверждения существующих тестов: `tests/prepare.rs` (предупреждение о скриптах от `prepare` больше
  не приходит — проверка переезжает в `tests/scripts.rs`), `tests/send.rs` (то же через `send`: теперь
  `ScriptsNotRun { reason: Disabled, .. }`), `tests/owner_end_to_end.rs` (счётчик «with script warning» считается
  по `script_steps`); во все литералы `SendInput` — `request_id` и `scripts`.

- [ ] **Step 1: Падающие тесты** (`tests/scripts.rs`; wiremock на `127.0.0.1`; хранилище в памяти; дерево
  «коллекция `Col` → папка `A` → папка `B` → запрос `Req`»; после `load_send_input` тест ставит `input.scripts =
  ScriptsMode::Run`). Консоль в этом файле сравнивается только через помощник `script_lines(&ScriptsReport) ->
  Vec<&ConsoleEntry>` — строки с `source != None`: Task 7 добавит в консоль строку приложения о сетевом запросе
  (`source == None`), и утверждения этой задачи не должны от неё зависеть. «Строк консоли нет», «консоль `x`»,
  «тексты консоли по порядку» ниже и в Task 8, 10 — всегда про `script_lines`:
  - `milestone_pre_request_sets_a_variable_and_a_test_checks_the_response` — переменная коллекции `host` = адрес
    сервера; окружение `Dev` выбрано; адрес запроса `{{host}}/login?u={{user}}`; pre-request запроса:
    `pm.variables.set('user', 'kubat'); pm.environment.set('stage', 'pre');`; test запроса:
    ```js
    pm.test('status is 200', function () { if (pm.response.code !== 200) throw new Error('code ' + pm.response.code); });
    var data = JSON.parse(responseBody);
    pm.collectionVariables.set('token', data.access_token);
    ```
    сервер отвечает `{"access_token":"T-1"}` только на `GET /login` с `u=kubat` → `result` — ответ 200; `tests ==
    [status is 200: passed, источник Req / Test]`; `changes == [Environment Set stage pre, Collection Set token
    T-1]`; `errors` пуст; предупреждения `UnresolvedVariables` нет;
  - `scripts_run_in_postman_order` — у `Col`, `A`, `B`, `Req` оба скрипта вида `console.log('<узел> <фаза>')` →
    тексты консоли по порядку: `Col pre`, `A pre`, `B pre`, `Req pre`, `Col test`, `A test`, `B test`, `Req test`;
    у каждой строки верные `node_id`, `node_kind`, `phase`; `at_ms` не убывает;
  - `variables_flow_from_script_to_script_and_into_the_request` — pre `Col`: `pm.environment.set('id', 7)`; pre
    `Req`: `pm.variables.set('path', 'p' + pm.environment.get('id'))`; адрес `{{host}}/{{path}}`, заголовок `X-Id:
    {{id}}`; test `Req`: `console.log(pm.variables.get('path'), typeof pm.environment.get('id'))` → сервер получил
    `GET /p7` с `X-Id: 7`; консоль `p7 number`; `changes == [Environment Set id "7"]`;
  - `pre_request_error_stops_everything` — pre `Col`: `pm.globals.set('before', '1');\nthrow new Error('nope');`;
    остальные скрипты пишут в консоль → сервер не получил ни одного запроса; `result` — `Err(SendError::Script)` с
    источником `Col / PreRequest`, `line == Some(2)`, текст `Error: nope`; `request` — `None`; строк консоли нет;
    в `changes` есть `Globals Set before 1`;
  - `test_error_keeps_the_response_and_the_next_scripts_run` — test `Col` бросает ошибку, test `Req`:
    `pm.test('still', () => {})` → `result` — ответ; одна ошибка (`Col / Test`); тест `still` пройден;
  - `no_response_no_tests` — адрес на закрытый порт → `Err(Http(Connect))`; строка консоли pre-request есть,
    строк test-скриптов нет;
  - `scripts_off_give_a_warning_and_run_nothing` — `Off(Disabled)` и `Off(NotAllowed { collection: "Col" })` →
    `warnings[0] == ScriptsNotRun { reason, nodes: ["Col", "Req"] }` (скрипты есть у `Col` и `Req`), `scripts ==
    ScriptsReport::default()`, запрос ушёл как набран; у запроса, все скрипты которого и его предков — пробелы и
    переводы строк, предупреждения нет;
  - `blank_scripts_change_nothing` — режим `Run`, скрипты `"  \n"` → `scripts == ScriptsReport::default()`, ответ
    получен;
  - `unconfirmed_files_stop_before_any_script` — неподтверждённый файл в теле и pre-request с `console.log` →
    `Err(Prepare(UnconfirmedFiles))`, консоль пуста;
  - `cancel_during_pre_request` — pre `while (true) {}`, отмена через 100 мс → `Err(SendError::Cancelled)` быстрее
    3 с, сервер не получил запросов;
  - `body_over_the_limit_fails_the_test_not_the_send` — сервер отдаёт 11 МиБ; test `pm.test('json', () =>
    pm.response.json())` → `result` — ответ с `decoded_size` 11 МиБ; тест провален с текстом про 10 MB;
  - `unresolved_is_counted_after_the_scripts` — адрес `{{made}}`, pre `pm.variables.set('made',
    pm.collectionVariables.get('host'))` → предупреждения `UnresolvedVariables` нет;
  - `draft_script_of_the_request_runs` — `load_send_input` с черновиком запроса, чей test-скрипт пишет в консоль →
    строка есть.
- [ ] **Step 2: Запустить** `cargo test -p rqm-runner` — новые тесты не собираются или падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные; `cargo tree -p rqm-runner -i tauri` — пакета нет.
- [ ] **Step 5: Commit** — «Этап 2: конвейер отправки — pre-request, подстановка, HTTP, test».

---

### Task 5: chai — `pm.expect`, утверждения ответа, асинхронные тесты, `tests[...]`

**Files:**
- Create: `crates/rqm-script/prelude/src/chai-entry.js`, `prelude/src/assertions.js`, `prelude/built/chai.js`
- Modify: `prelude/package.json` (+ `"dependencies": { "chai": "4.4.1" }`), `package-lock.json`, `build.mjs`
  (второй вход `src/chai-entry.js` → `built/chai.js`), `prelude/src/test.js`, `legacy.js`, `response.js`,
  `index.js`, `built/prelude.js`; `crates/rqm-script/src/bundle.rs` (`Bundle::Chai`), `src/host.rs`
- Test: `crates/rqm-script/tests/expect.rs`, `tests/bundle.rs`, `tests/tamper.rs`

**Interfaces:**
- Consumes: `bundle::load`, мост `__host` (Task 2); объект ответа и `pm.test` (Task 3).
- Produces (JavaScript; публичный Rust-интерфейс не меняется): `pm.expect`; свойство `to` у объекта ответа;
  `pm.test` с `done` и с Promise; глобальный объект `tests`. `pub(crate) enum Bundle { Prelude, Chai }`;
  примитив хоста `load_bundle(name)`.

**Правила:**
- `built/chai.js` — chai 4.4.1 и плагин `assertions.js`, подключённый через `chai.use`. Грузится в контекст
  **лениво**: при первом чтении `pm.expect` или `to` у объекта ответа. Скрипт, который chai не трогает, его не
  грузит.
- Загрузку делает один обобщённый примитив хоста `load_bundle(name)` (`src/host.rs`): имя — строка из списка
  известных bundle (в этой задаче `"chai"`; Task 9 добавит `"lodash"` — только строкой в списке и вариантом
  `Bundle`); неизвестное имя — исключение JS; повторный вызов для уже загруженного в этот контекст bundle ничего
  не делает. Примитив вызывает `bundle::load(ctx, …)` прямо во время скрипта — это вызов в JS из примитива:
  замок состояния хоста в этот момент не удерживается (правило движка № 9), секундомер стоит.
- `pm.expect` — `chai.expect`. `pm.response.to` — `chai.expect(pm.response).to`.
- Плагин — свой код; имена и тексты сообщений — как в `chai-postman` 2.0.1 (сверено по его исходнику):

  | Утверждение | Проверка | Сообщение провала (в скобках — при отрицании) |
  |---|---|---|
  | `status(x)`, `x` — число | `code === x` | `expected response to have status code #{exp} but got #{act}` (`expected response to not have status code #{act}`) |
  | `status(x)`, иначе | `reason() === x` | `expected response to have status reason #{exp} but got #{act}` (`expected response to not have status reason #{act}`) |
  | `statusCode(n)`, `statusReason(s)` | как две строки выше | те же |
  | `statusCodeClass(n)`; свойства `info` (1), `success` (2), `redirection` (3), `clientError` (4), `serverError` (5) | `Math.floor(code / 100) === n` | `expected response code to be #{exp}XX but found #{act}` (`expected response code to not be #{exp}XX`) |
  | `error` | класс 4 или 5 | `expected response code to be 4XX or 5XX but found #{act}` (`expected response code to not be 4XX or 5XX`) |
  | `ok` на объекте ответа | фраза без учёта регистра `OK` и код 200 | как у `status` |
  | `accepted` (202, `Accepted`), `withoutContent` (204, `No Content`), `badRequest` (400, `Bad Request`), `unauthorised` (401, `Unauthorised`), `unauthorized` (401, `Unauthorized`), `forbidden` (403, `Forbidden`), `notFound` (404, `Not Found`), `notAcceptable` (406, `Acceptable` — так в Postman), `rateLimited` (429, `Too Many Requests`) | фраза в верхнем регистре равна названной в верхнем регистре, затем код | как у `status`: сначала фраза, потом код |
  | `header(name)` / `header(name, value)` | заголовок есть / равен | `expected response to have header with key 'NAME'`; `expected 'NAME' response header to be #{exp} but got #{act}` |
  | `withBody`; `body()` без аргумента | тело не пусто | `expected response to have content in body` |
  | `json`; `jsonBody()` без аргумента | `json()` не бросает | `expected response body to be a valid json but got error <текст>` |
  | `body(RegExp)` | совпадение с текстом | `expected response body text to match #{exp} but got #{act}` |
  | `body(объект)` | глубокое равенство с `json()` | `expected response body json to equal #{exp} but got #{act}` |
  | `body(другое)` | равенство с `text()` | `expected response body to equal #{exp} but got #{act}` |
  | `jsonBody(path)` | путь есть | `expected #{act} in response to contain property #{exp}` |
  | `jsonBody(path, value)` | значение по пути глубоко равно | `expected response body json at "PATH" to contain #{exp} but got #{act}` |

  Путь — как у lodash: имена через точку и индексы `[n]` (свой разбор, lodash не грузится); глубокое равенство —
  `chai.util.eql`. «Объект ответа» — созданный фабрикой ответа прелюдии; утверждение ответа на другом значении
  проваливается с `expecting a postman response object but got #{this}`; `ok` на другом значении — обычный `ok`
  из chai.
- Асинхронные тесты: `fn.length > 0` → `fn(done)`, тест записывается при `done(err)`; синхронное исключение —
  провал; `done` не вызван к концу скрипта → провал с `error: "done() was not called"`. Функция вернула thenable
  → тест записывается, когда он завершится; отклонение — провал. Порядок тестов — порядок записи.
- `tests` — глобальный объект. Когда скрипт закончился (нормально или исключением, но не `Timeout`, `Memory`,
  `Cancelled`), каждая его собственная пара становится тестом **после** тестов `pm.test` этого скрипта: `passed
  = Boolean(value)`; у проваленного `error` — `AssertionError: expected <v> to be truthy`, где `<v>` — `''` для
  пустой строки, иначе `String(value)`.

- [ ] **Step 1: Падающие тесты** (`tests/expect.rs`; каждое утверждение — в своём `pm.test`):
  - `chai_messages_match_postman` — `pm.expect(1).to.equal(2)` → `AssertionError: expected 1 to equal 2`;
    `pm.expect({ a: [1, { b: 2 }] }).to.deep.equal({ a: [1, { b: 3 }] })` → `AssertionError: expected { a: [ 1, {
    b: 2 } ] } to deeply equal { a: [ 1, { b: 3 } ] }`; `pm.expect('abc').to.be.a('number')` → `AssertionError:
    expected 'abc' to be a number`; `pm.expect({ a: 1 }).to.have.property('b')` → `AssertionError: expected { a: 1
    } to have property 'b'`; `pm.expect([1, 2]).to.include(3)` → `AssertionError: expected [ 1, 2 ] to include 3`;
    `pm.expect(200).to.be.oneOf([201, 202])` → `AssertionError: expected 200 to be one of [ 201, 202 ]`;
    `pm.expect(1, 'with message').to.be.below(0)` → `AssertionError: with message: expected 1 to be below +0`;
    `pm.expect('').to.be.ok` → `AssertionError: expected '' to be truthy`;
  - `common_chains_pass` — один тест: `eql`, `be.an('array')`, `have.property('a', 1)`, `be.true`, `be.null`,
    `exist`, `be.above`, `be.at.least`, `be.within`, `have.lengthOf`, `match`, `have.keys`, `have.members`,
    `not.be.empty`, `throw` → пройден;
  - `response_assertions_pass` — ответ 200 `OK`, `Content-Type: application/json`, тело `{"user":{"id":7,"tags":
    ["a"]}}`; один тест: `pm.response.to.have.status(200); pm.response.to.have.status('OK'); pm.response.to.be.ok;
    pm.response.to.be.success; pm.response.to.not.be.error; pm.response.to.have.header('content-type');
    pm.response.to.have.header('Content-Type', 'application/json'); pm.response.to.be.json;
    pm.response.to.be.withBody; pm.response.to.have.body(/user/); pm.response.to.have.jsonBody('user.id', 7);
    pm.response.to.have.jsonBody('user.tags[0]', 'a'); pm.response.to.have.statusCode(200);
    pm.response.to.have.statusCodeClass(2); pm.response.to.not.have.status(404)` → пройден. Ответ 404 `Not Found`:
    `pm.response.to.be.notFound; pm.response.to.be.clientError; pm.response.to.be.error; pm.response.to.not.be.ok`
    → пройден;
  - `response_assertions_fail_with_postman_messages` — на ответе 200: `to.have.status(201)` → `AssertionError:
    expected response to have status code 201 but got 200`; `to.have.status('Created')` → `AssertionError:
    expected response to have status reason 'Created' but got 'OK'`; `to.be.clientError` → `AssertionError:
    expected response code to be 4XX but found 200`; `to.have.header('X-No')` → `AssertionError: expected response
    to have header with key 'X-No'`; `to.be.notFound` → `AssertionError: expected response to have status reason
    'NOT FOUND' but got 'OK'`; `to.have.jsonBody('user.name')` → текст содержит `in response to contain property
    'user.name'`; `pm.expect({}).to.have.status(200)` → текст содержит `expecting a postman response object`;
  - `async_tests` — `pm.test('done ok', function (done) { Promise.resolve().then(() => done()) }); pm.test('done
    err', function (done) { done(new Error('late')) }); pm.test('never', function (done) {}); pm.test('promise
    ok', async () => { await Promise.resolve() }); pm.test('promise bad', async () => { throw new Error(
    'rejected') });` → `done ok` пройден; `done err` провален, `Error: late`; `promise ok` пройден; `promise bad`
    провален, `Error: rejected`; `never` провален, `done() was not called`; `errors` пуст;
  - `legacy_tests_object` — `tests['is ok'] = responseCode.code === 200; tests.zero = 0; tests['empty'] = '';
    pm.test('first', () => {});` → по порядку: `first` пройден; `is ok` пройден; `zero` провален, `AssertionError:
    expected 0 to be truthy`; `empty` провален, `AssertionError: expected '' to be truthy`;
  - `legacy_tests_survive_an_error` — `tests.a = true; throw new Error('x')` → тест `a` пройден и одна ошибка;
  - `legacy_tests_object_can_be_replaced` — `tests = { a: true }; tests.b = false` (присваивание без `var`) → тест
    `a` пройден, `b` провален;
  - `chai_loads_wherever_it_is_first_touched` — первое чтение `pm.expect` (а) внутри `pm.test`, (б) внутри
    callback'а `Promise.resolve().then(...)`, (в) первое чтение `pm.response.to` внутри `pm.test` → тесты
    пройдены, `errors` пуст, ни одной ошибки вида `Internal` (повторный вход в хост не паникует и не виснет);
  - `load_bundle_refuses_unknown_names` — модульный тест примитива: имя `"fs"` → ошибка, контекст цел.
  `tests/tamper.rs`: оба теста дополнить `pm.expect` и `pm.response.to.*` (подмена `Function.prototype.call /
  apply` сделана до первой загрузки chai — chai при загрузке пользуется ими законно; примитивов хоста среди
  собранных функций по-прежнему нет).
  `tests/bundle.rs`: тест суммы переименовать в `built_files_match_their_sources` — проверяет каждый файл в
  `built/`.
- [ ] **Step 2: Запустить** `cargo test -p rqm-script` — новые тесты падают.
- [ ] **Step 3: Реализовать**; в `prelude/`: `npm install`, `npm run build`. В отчёт — размер `built/chai.js`.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: chai 4.4.1 — pm.expect, утверждения ответа, асинхронные тесты, tests[...]».

---

### Task 6: Асинхронность — таймеры, конец скрипта, `pm.sendRequest` в движке

**Files:**
- Create: `crates/rqm-script/prelude/src/async.js`, `prelude/src/send-request.js`
- Modify: `prelude/src/index.js`, `response.js`, `built/prelude.js`; `crates/rqm-script/src/lib.rs`,
  `src/engine.rs`, `src/host.rs`, `src/budget.rs`
- Test: `crates/rqm-script/tests/async_run.rs`, `tests/send_request.rs`, `tests/tamper.rs`

**Interfaces:**
- Consumes: цикл заданий, сторож и флаг `abandoned` Task 1; фабрика объекта ответа (Task 3, 5); обёртка
  `wellFormed` и флаг предела памяти хоста (Task 2); `Runtime::set_host_promise_rejection_tracker` (вызывается дважды для Promise, к которому `catch` приделали
  позже: сначала `is_handled = false`, потом `true`).
- Produces:

```rust
/// The most a script may wait for its timers.
pub const TIMER_WAIT_LIMIT: Duration = Duration::from_secs(60);
/// The most `pm.sendRequest` calls of one script.
pub const MAX_SEND_REQUESTS: usize = 50;
/// The most one `pm.sendRequest` may carry: UTF-8 bytes of the method, the address, the headers and the body.
pub const MAX_REQUEST_BYTES: usize = 10 * 1_048_576;
/// The most the requests waiting in the queue of one script may hold together.
pub const MAX_QUEUED_REQUEST_BYTES: usize = 32 * 1_048_576;
// Limits: + pub timer_wait: Duration        // Default: TIMER_WAIT_LIMIT
// ConsoleLevel: + Network

/// What the scripts ask of the application. The host moves to the thread of the scripts and may outlive
/// `run_with_host` there (a thread the watchdog gave up on), hence `'static`; an abandoned script never
/// reaches it.
pub trait Host { /// Sends the request and blocks until the answer; `Err` — the text of the network error.
                 /// Must return soon after the token of the run is cancelled.
                 fn send_request(&mut self, request: OutgoingRequest) -> Result<ScriptResponse, String>; }
#[derive(Debug, Clone, PartialEq)]
pub struct OutgoingRequest { pub method: String, pub url: String, pub headers: Vec<(String, String)>, pub body: OutgoingBody }
#[derive(Debug, Clone, PartialEq)]
pub enum OutgoingBody { None, Raw(String), UrlEncoded(Vec<(String, String)>), /// Text parts only.
                        FormData(Vec<(String, String)>) }
pub fn run_with_host(job: Job, host: Box<dyn Host + Send>, cancel: &CancellationToken) -> Report;
// run(job, cancel) — то же с хостом, который на любой запрос отвечает Err("pm.sendRequest is not available")
```

**Правила:**
- Хост уходит в поток скрипта владением (поток отсоединённый — Task 1), поэтому `Box<dyn Host + Send>`, а не
  ссылка: всё, что хост должен отдать наружу, он отдаёт через свои каналы или `Arc`. Результаты скриптов
  (переменные, журнал, запрос) возвращаются только отчётом.
- Цикл одного скрипта: (1) выполнить все готовые задания; (2) отмена, таймаут, флаг предела памяти хоста или
  `abandoned` — стоп (при `abandoned` хост не вызывается и таймеры не ждутся — сетевой запрос не должен уйти
  после того, как отправке уже вернули ошибку); (3) есть запросы `pm.sendRequest` в очереди — взять самый
  ранний: секундомер на паузу (срок сторожа снят), `host.send_request`, секундомер дальше, строка консоли, ответ
  — в прелюдию; (4) иначе есть таймеры — ждать ближайший (секундомер на паузе; сон кусками
  не длиннее 10 мс с проверкой токена), вызвать; (5) иначе: итоговый Promise завершён — скрипт окончен, не
  завершён — ошибка `never finishes` из Task 1. Скрипт окончен, только когда Promise завершён и не осталось ни
  таймеров, ни запросов.
- Суммарное ожидание таймеров скрипта больше `limits.timer_wait` → `Timeout` с текстом `the script waited for
  timers longer than {N} ms`.
- Таймеры: `setTimeout(fn, ms, ...args)`, `setInterval`, `setImmediate(fn, ...args)`, `clearTimeout`,
  `clearInterval`, `clearImmediate`. Номер — положительное целое. Задержка — `Number(ms)`; не число или меньше
  нуля — 0. `fn` не функция — таймер не заводится. Функции лежат в таблице прелюдии; Rust хранит только «номер,
  срок, период».
- Первая необработанная ошибка асинхронной части — исключение в callback'е таймера или `pm.sendRequest`, либо
  отклонённый Promise без обработчика к моменту, когда ждать больше нечего, — становится ошибкой скрипта (со
  строкой и столбцом из её стека) и останавливает скрипт; оставшиеся таймеры и запросы отбрасываются. Список
  «отклонён и пока не обработан» ведёт прелюдия в своём `Map` (трекер зовёт её функцию — проверено опытом);
  Rust JS-значений не хранит. Отклонение самого итогового Promise скрипта тоже приходит в трекер — в список
  смотрят, только когда итоговый Promise завершился успешно, иначе одна ошибка была бы названа дважды.
- `pm.sendRequest(req, callback)` возвращает `pm`; `pm.sendRequest(req)` возвращает Promise.
  - `req` строкой — адрес, метод `GET`. Объектом: `url` (строка или объект с `raw`), `method` (по умолчанию
    `GET`), `header` — объект `{ имя: значение }`, массив `{ key, value, disabled }` или строка из строк `Имя:
    значение`; `body`: `{ mode: 'raw', raw }` (не строка → `JSON.stringify`), `{ mode: 'urlencoded', urlencoded }`
    (массив `{ key, value, disabled }` или строка `a=1&b=2`), `{ mode: 'formdata', formdata }` (массив `{ key,
    value, disabled }`). Отключённые строки не отправляются.
  - Файлы запрещены: `mode: 'file'` и части formdata с `type: 'file'` или с `src` вырезаются, в консоль — `Warn`
    `uploading files from scripts is not allowed`. Неизвестный `mode` — запрос уходит без тела, `Warn`
    `pm.sendRequest: body mode '<mode>' is not supported`.
  - `{{переменные}}` в запросе `pm.sendRequest` **не подставляются**: адрес, заголовки и тело уходят как дал
    скрипт. Так в Postman (сверено по `postman-runtime` 7.56.3, `event.command.js`: запрос скрипта идёт сразу в
    `httprequest`, минуя `resolveVariables`); нужна подстановка — скрипт зовёт `pm.variables.replaceIn(...)`.
  - Все строки запроса проходят `wellFormed` (Task 2).
  - Адреса нет → ошибка `sendrequest: nothing to request` (в callback либо отклонением Promise).
  - Запросы скрипта исполняются по одному, в порядке вызова. Пределы считает примитив постановки в очередь
    (правило движка № 6): вызов сверх `MAX_SEND_REQUESTS` в одном скрипте → `pm.sendRequest` бросает `Error:
    pm.sendRequest: more than 50 requests in one script`; запрос больше `MAX_REQUEST_BYTES` (метод, адрес,
    заголовки и тело вместе; длина — до копирования в Rust) в очередь не попадает, хост не вызывается, ошибка
    `pm.sendRequest: the request is larger than 10 MB` приходит в callback либо отклонением Promise; запросы,
    ждущие в очереди, вместе больше `MAX_QUEUED_REQUEST_BYTES` → флаг предела памяти хоста, ошибка `Memory` с
    текстом `the script queued more than 32 MiB of requests`.
  - Ответ: `callback(null, response)`; `response` — объект той же фабрики, что `pm.response` (`code`, `status`,
    `headers`, `text()`, `json()`, `responseTime`, `responseSize`, `to`). Ошибка хоста: `callback(error)` с
    `error.message` — текст хоста; Promise отклоняется той же ошибкой.
  - После каждого запроса — строка консоли `Network` от имени скрипта: `{method} {url} → {code} {status} ({N}
    ms)` либо `{method} {url} → error: {текст}`.
- Отмена во время ожидания таймера или хоста — ошибка `Cancelled`.

- [ ] **Step 1: Падающие тесты.**
  `tests/async_run.rs`:
  - `timers_fire_in_order` — `setTimeout(() => console.log('b'), 20); setTimeout(() => console.log('a'), 5);
    setImmediate(() => console.log('i')); console.log('sync'); await new Promise((r) => setTimeout(r, 40));
    console.log('end')` → `sync`, `i`, `a`, `b`, `end`;
  - `script_waits_for_its_timers` — скрипты `["setTimeout(() => pm.globals.set('late', '1'), 30)", "console.log(
    pm.globals.get('late'))"]` → в `changes` есть `Set late`, консоль `1`;
  - `interval_and_clear` — `let n = 0; const id = setInterval(() => { n++; console.log(n); if (n === 3)
    clearInterval(id) }, 5); clearTimeout(setTimeout(() => console.log('no'), 5))` → `1`, `2`, `3`;
  - `timer_arguments_and_bad_input` — `setTimeout((a, b) => console.log(a + b), 0, 1, 2); setTimeout('text', 0);
    setTimeout(() => console.log('neg'), -5)` → `3`, `neg`;
  - `waiting_is_not_execution_time` — предел времени 200 мс, `await new Promise((r) => setTimeout(r, 400));
    console.log('ok')` → `errors` пуст;
  - `endless_interval_hits_the_timer_wait_limit` — `timer_wait` 300 мс, `setInterval(() => {}, 10)` → `Timeout`,
    `the script waited for timers longer than 300 ms`;
  - `cancel_while_waiting_for_a_timer` — `await new Promise((r) => setTimeout(r, 30000))`, отмена через 100 мс →
    `Cancelled` быстрее 2 с;
  - `error_in_a_timer_is_the_error_of_the_script` — `setTimeout(() => { throw new Error('in timer') }, 5);\n
    setTimeout(() => console.log('never'), 50)` → `Exception`, `Error: in timer`, строка 1; строки `never` нет;
  - `unhandled_rejection_is_an_error_and_a_handled_one_is_not` — `Promise.reject(new Error('lost'))` →
    `Exception`, `Error: lost`; `const p = Promise.reject(new Error('caught')); await Promise.resolve(); p.catch(
    () => {})` → `errors` пуст;
  - `done_from_a_timer` — `pm.test('later', function (done) { setTimeout(done, 10) })` → пройден.
  `tests/send_request.rs` (хост теста отвечает по таблице адресов, а полученные запросы складывает в
  `Arc<Mutex<Vec<OutgoingRequest>>>`, общий с тестом, — сам хост уходит в `run_with_host` владением; `…/a` → 200
  `OK` `{"v":1}`, `…/b` → 201 `Created`):
  - `callback_and_promise_forms` — `pm.sendRequest('http://x/a', function (err, res) { console.log(err === null,
    res.code, res.json().v) }); const r = await pm.sendRequest({ url: 'http://x/b', method: 'POST', header: {
    'X-K': 'v' }, body: { mode: 'raw', raw: JSON.stringify({ a: 1 }) } }); console.log(r.code, r.status)` → хост
    получил `GET http://x/a` без тела, затем `POST http://x/b`, `[("X-K", "v")]`, `Raw("{\"a\":1}")`; в консоли
    есть `true 200 1` и `201 Created`;
  - `header_and_body_forms` — `header` массивом `[{ key: 'A', value: '1' }, { key: 'B', value: '2', disabled: true
    }]` → `[("A", "1")]`; строкой `'A: 1\nB: 2'` → обе пары; `urlencoded` массивом и строкой `'a=1&b=2'` →
    `UrlEncoded([("a", "1"), ("b", "2")])`; `formdata` → `FormData`; `raw: { a: 1 }` → `Raw("{\"a\":1}")`;
  - `files_are_cut_out` — `body: { mode: 'file', file: { src: 'C:/secret.txt' } }` → `OutgoingBody::None` и строка
    `Warn` `uploading files from scripts is not allowed`; `formdata: [{ key: 'f', type: 'file', src: 'C:/x' }, {
    key: 't', value: '1' }]` → `FormData([("t", "1")])` и та же строка;
  - `variables_are_not_substituted` — globals `host=http://h`; `pm.variables.set('id', '7'); pm.sendRequest({ url:
    'http://h/u/{{id}}', header: { 'X-Id': '{{id}}' }, body: { mode: 'raw', raw: '{"t":"{{id}}"}' } })` → хост
    получил `OutgoingRequest` с литералом `{{id}}` в адресе (`http://h/u/{{id}}`), в заголовке и в теле; второй
    запрос того же скрипта с `raw: pm.variables.replaceIn('{"t":"{{id}}"}')` → тело `{"t":"7"}`;
  - `too_big_request_is_refused` — `pm.sendRequest({ url: 'http://x/a', body: { mode: 'raw', raw: 'x'.repeat(11 *
    1024 * 1024) } }, (err) => console.log(err.message))` → `pm.sendRequest: the request is larger than 10 MB`,
    хост не вызван; тело ровно из 9 МиБ уходит;
  - `queued_requests_are_capped` — пять вызовов подряд с телом по 8 МиБ без `await` → ошибка `Memory`, текст `the
    script queued more than 32 MiB of requests`; хост получил не больше четырёх запросов;
  - `abandoned_script_does_not_reach_the_host` — `Limits { time: 100 мс, grace: 100 мс, .. }`, `(10n **
    300000n).toString(); pm.sendRequest('http://x/late')` → `Timeout` быстрее 1,5 с; тест ждёт (до 30 с), пока
    `abandoned_threads()` не вернётся к `0`; хост не получил ни одного запроса;
  - `network_error_reaches_the_callback_and_rejects_the_promise` — хост отвечает `Err("connect refused")`:
    `pm.sendRequest(url, (err, res) => console.log(err.message, res))` → `connect refused undefined`; `await
    pm.sendRequest(url)` без `catch` → `Exception`, `Error: connect refused`;
  - `requests_run_one_at_a_time_and_hold_the_script` — `pm.sendRequest('http://x/1'); pm.sendRequest(
    'http://x/2')` без callback'ов и `await` → хост получил `/1`, затем `/2`, оба до конца `run`;
  - `network_lines_in_the_console` — после успешного запроса строка `Network`, текст по шаблону `^GET http://x/a
    → 200 OK \(\d+ ms\)$`; после ошибки — `GET http://x/a → error: connect refused`;
  - `too_many_requests` — цикл из 51 вызова → `Exception` с `more than 50 requests`;
  - `nothing_to_request` — `pm.sendRequest({}, (err) => console.log(err.message))` → `sendrequest: nothing to
    request`;
  - `answer_is_a_response_object` — `pm.sendRequest('http://x/a', (e, res) => { pm.test('inner', () => {
    pm.expect(res).to.have.status(200); res.to.be.ok; pm.expect(res.headers.count()).to.be.a('number') }) })` →
    тест `inner` пройден;
  - `waiting_for_the_host_is_not_execution_time` — хост спит 400 мс, предел времени 200 мс → `errors` пуст;
  - `cancel_while_the_host_answers` — хост ждёт отмены токена и отвечает `Err("cancelled")` → ошибка `Cancelled`;
  - `run_without_a_host_refuses` — `run(job, cancel)`, `await pm.sendRequest('http://x')` → `Exception`, `Error:
    pm.sendRequest is not available`.
  `tests/tamper.rs`: оба теста дополнить таймерами и `pm.sendRequest` (51 вызов при подменённых встроенных
  объектах → хост получил не больше 50 запросов). Модульный тест примитива очереди: 51-й запрос и запрос больше
  `MAX_REQUEST_BYTES` — отказ; номер таймера или callback'а, которого Rust не выдавал, — ошибка, не паника.
- [ ] **Step 2: Запустить** `cargo test -p rqm-script` — новые тесты падают.
- [ ] **Step 3: Реализовать**; `npm run build` в `prelude/`.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: таймеры, завершение скрипта и pm.sendRequest в движке».

---

### Task 7: `pm.sendRequest` в конвейере — мост к `HttpEngine`

**Files:**
- Create: `crates/rqm-runner/src/script_host.rs`
- Modify: `crates/rqm-runner/src/scripts.rs`, `src/send.rs`, `src/prepare.rs` (`form_encode` — `pub(crate)`)
- Test: `crates/rqm-runner/tests/scripts.rs`

**Interfaces:**
- Consumes: `rqm_script::{run_with_host, Host, OutgoingRequest, OutgoingBody, ScriptResponse, ResponseBody,
  BODY_LIMIT, ConsoleLevel::Network}` (Task 6); `HttpEngine::send(request, cancel)`, `HttpRequest { method, url,
  headers, body, options }`, `HttpBody::{None, Bytes { data, default_content_type }, Multipart(Vec<MultipartPart>)}`,
  `MultipartPart { name, value: PartValue::Text(..), content_type }`, `SendOptions` (`Default`: без таймаута,
  редиректы включены, до 10, проверка TLS, системный прокси).
- Produces: поведение `send`; новых публичных имён нет.

**Правила:**
- Фаза скриптов: `tokio::task::spawn_blocking` вызывает `rqm_script::run_with_host` с хостом (`Box`, владеет
  только отправителем канала), который кладёт в `tokio::sync::mpsc::unbounded_channel` сообщение `HostCall::
  Request(OutgoingRequest, std::sync::mpsc::Sender<Result<ScriptResponse, String>>)` и ждёт ответа; когда
  `run_with_host` вернулся, та же задача кладёт в канал `HostCall::Done(Report)` (у задачи своя копия
  отправителя). Асинхронная часть `send` принимает из канала: на `Request` делает `engine.send(request,
  cancel.clone()).await` и отвечает; на `Done` — фаза окончена, приёмник уничтожается. Ждать закрытия канала
  нельзя: хост живёт в потоке скрипта и у брошенного сторожем потока не уничтожается. Поздний запрос брошенного
  потока до сети не доходит: его не пускает флаг `abandoned` (Task 1, 6), а отправка в канал без приёмника —
  ошибка, на которую хост отвечает `Err("cancelled")`. Канал закрылся без `Done` (задача упала) — `ScriptFailure`
  вида `Internal`, как в Task 4. `engine` поток скрипта не получает.
- `OutgoingRequest` → `HttpRequest`: метод как дан; к адресу без `://` спереди добавляется `http://`; заголовки
  как даны; тело: `Raw` → `Bytes` с типом по умолчанию `text/plain`; `UrlEncoded` → `Bytes` из пар
  `form_encode(key)=form_encode(value)` через `&`, тип по умолчанию `application/x-www-form-urlencoded`;
  `FormData` → `Multipart` из текстовых частей; настройки — `SendOptions { timeout: input.settings.timeout,
  verify_tls: input.settings.verify_tls, proxy: input.settings.proxy.clone(), ..SendOptions::default() }`.
- Ответ → `ScriptResponse` как у основного запроса, но тело — `ResponseBody::Bytes`: из файла читается не больше
  `BODY_LIMIT + 1` байт, затем файл **удаляется** (и когда чтение не удалось). `HttpError` → `Err(текст ошибки)`.
- Скрипты и auth коллекции к запросу скрипта не применяются; в историю он не пишется; cookies — общие (тот же
  движок).
- Когда скрипты этой отправки исполняются (режим `Run` и есть хоть один шаг), после основного запроса в консоль
  добавляется строка приложения: `ConsoleEntry { source: None, level: Network, text }` с текстом `{method}
  {url} → {status} {status_text} ({N} ms)` или `{method} {url} → error: {текст}` — метод и адрес как ушли в сеть.

- [ ] **Step 1: Падающие тесты** (`tests/scripts.rs`; движок — настоящий `ReqwestEngine` с общим `CookieJar`,
  как в `tests/send.rs`):
  - `token_is_fetched_in_pre_request` — pre-request запроса:
    ```js
    const res = await pm.sendRequest({ url: pm.collectionVariables.get('host') + '/token', method: 'POST',
      header: { 'Content-Type': 'application/json' }, body: { mode: 'raw', raw: JSON.stringify({ user: 'u' }) } });
    pm.environment.set('token', res.json().access_token);
    ```
    заголовок запроса `Authorization: Bearer {{token}}`; сервер на `POST /token` с телом `{"user":"u"}` отвечает
    `{"access_token":"T-9"}` → `GET /data` пришёл с `Authorization: Bearer T-9`; `changes == [Environment Set
    token T-9]`;
  - `send_request_shares_the_cookies` — `/login` ставит `Set-Cookie: sid=1`; pre-request: `await pm.sendRequest(
    host + '/login')` → основной запрос пришёл с `Cookie: sid=1`;
  - `bodies_of_script_requests_are_removed` — после отправки в папке тел движка ровно один файл (тело основного
    ответа);
  - `urlencoded_and_formdata_bodies` — сервер получил тело `a=1&b=x+y` с `Content-Type:
    application/x-www-form-urlencoded`; для formdata — `multipart/form-data` с частью `t`;
  - `network_error_in_a_script_request_does_not_stop_the_send` — адрес на закрытый порт, callback пишет
    `err.message` в консоль → строка есть, основной запрос ушёл;
  - `console_has_network_lines_in_order` — строка `Network` скрипта `POST …/token → 200 OK (… ms)`, затем строка
    приложения (`source == None`) `GET …/data → 200 OK (… ms)`;
  - `cancel_during_a_script_request` — `/token` отвечает через 30 с, отмена через 100 мс →
    `Err(SendError::Cancelled)` быстрее 3 с;
  - `script_request_uses_the_application_timeout` — `settings.timeout` 200 мс, `/slow` отвечает через 2 с;
    pre-request: `try { await pm.sendRequest(host + '/slow') } catch (e) { console.log('failed') }` → строка
    `failed` есть;
  - `big_answer_of_a_script_request` — `/big` отдаёт 11 МиБ; `try { res.text() } catch (e) { console.log(
    e.message) }` → `response body is larger than 10 MB and is not available to scripts`.
- [ ] **Step 2: Запустить** `cargo test -p rqm-runner --test scripts` — новые тесты падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: pm.sendRequest через общий HttpEngine, сетевые строки консоли».

---

### Task 8: `pm.request`, `postman.*`, `pm.execution`

**Files:**
- Create: `crates/rqm-script/prelude/src/request.js`
- Modify: `prelude/src/lists.js`, `legacy.js`, `index.js`, `built/prelude.js`; `crates/rqm-script/src/lib.rs`,
  `src/host.rs`, `src/engine.rs`; `crates/rqm-runner/src/scripts.rs`, `src/send.rs`, `src/prepare.rs`
  (`query_encode` — `pub(crate)`, `Prepared.script_body`); `src-tauri/src/send.rs` (ветка `SendError::Skipped` →
  `SendErrorKind::Other`, временно до Task 11)
- Test: `crates/rqm-script/tests/request.rs`, `tests/tamper.rs`, `crates/rqm-runner/tests/scripts.rs`

**Interfaces:**
- Consumes: класс списка (Task 3), области переменных (Task 2), `prepare_with` (Task 4); `rqm_model::{
  RequestContent { method, url, query, headers, body, .. }, Param { key, value, enabled, postman_extra }, Body::{
  None, Raw { text, language }, UrlEncoded(Vec<Param>), FormData(..), Binary { .. }, GraphQl { .. } }, RawLanguage }`.
- Produces:

```rust
// crates/rqm-script
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScriptRequest { pub method: String,
                           /// The address with its query (the query stands before the `#fragment`). Pre-request:
                           /// as typed, variables are not substituted. Test: the address that was sent.
                           pub url: String, pub headers: Vec<ScriptPair>, pub body: ScriptBody }
#[derive(Debug, Clone, PartialEq)] pub struct ScriptPair { pub key: String, pub value: String, pub disabled: bool }
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ScriptBody { #[default] None, Raw(String), UrlEncoded(Vec<ScriptPair>),
                      /// The text fields of a form-data body (file parts are left out): a script reads them in
                      /// the legacy `request.data`, sees the mode `formdata` and cannot edit the body.
                      FormData(Vec<ScriptPair>),
                      /// A body a script sees only the mode of and cannot edit: `file`, `graphql`.
                      Other(String) }
// Job:    + pub request: ScriptRequest
// Report: + pub request: ScriptRequest,     // after the last script that ran; the caller reads it after PreRequest only
//           pub skip_request: bool          // pm.execution.skipRequest() was called; never true in Phase::Test
// crates/rqm-runner
pub enum SendError { /* … */ /// A pre-request script called `pm.execution.skipRequest()`; nothing was sent.
                     #[error("the request was skipped by a script")] Skipped }
// crates/rqm-runner/src/prepare.rs
pub struct Prepared { /* http, warnings */
                      /// The body as a test script sees it: substituted, before pruning and before encoding.
                      pub(crate) script_body: rqm_script::ScriptBody }
pub(crate) fn query_encode(text: &str, is_key: bool) -> String;   // было приватным; тело не меняется
```

**Правила прелюдии:**
- `pm.request.method` — чтение и запись.
- `pm.request.url` — объект над строкой адреса: `toString()` и `getRaw()` — строка как есть; `getHost()` — хост
  без схемы и порта; `getPath()`; `getQueryString()` — без `?`; `getPathWithQuery()`; `query` — список пар,
  разобранный из строки (правка списка переписывает query в строке, фрагмент `#…` остаётся в конце);
  `addQueryParams(строка 'a=1&b=2' | массив { key, value })`; `removeQueryParams(имя | массив имён)`. Разбор
  строки: схема `^[a-z][a-z0-9+.-]*://`, хост с портом — до первого `/`, `?` или `#`, путь — до `?` или `#`,
  query — до `#`; `{{…}}` — обычные символы. `pm.request.url = x` заменяет адрес на `String(x)`: объект
  `pm.request.url` один на скрипт, присваивание (сеттер на `pm.request`) меняет его содержимое, а не подменяет
  объект — заглушки Task 9 на нём остаются. Запись в запрос идёт через примитивы хоста; в `Phase::Test` они
  отказывают (правило движка № 6) — прелюдия в test меняет только свою копию.
- Список (`lists.js`) получает правку: `add({ key, value })` или `add('Имя: значение')` — в конец; `upsert({ key,
  value })` — меняет значение последней включённой пары с именем, иначе добавляет; `remove(name)` — убирает все
  пары с именем. У списков запроса `get`, `has`, `toObject`, `count` не видят отключённые пары; `all()` и
  `each()` отдают все, у отключённых `disabled: true`. Имена заголовков — без учёта регистра, имена параметров —
  с учётом.
- `pm.request.headers` — такой список; `pm.request.addHeader(x)`, `removeHeader(name)`, `upsertHeader(x)`.
- `pm.request.body`: `mode` (`raw`, `urlencoded`, `formdata`, `file`, `graphql`; без тела — `undefined`); `raw`
  — чтение и запись (запись при теле «нет» делает его raw); `urlencoded` — список; `toString()` — текст raw,
  для urlencoded — включённые пары `a=1&b=2`, иначе пустая строка; `isEmpty()`; `update(x)` — строка → raw; `{
  mode: 'raw', raw }`; `{ mode: 'urlencoded', urlencoded: [...] }`. Любой другой `mode` в `update` не
  применяется, в консоль `Warn` `pm.request.body: mode '<mode>' cannot be set from a script`. У тела `Other`
  запись `raw` и правка `urlencoded` не применяются, `Warn` `pm.request.body: a <mode> body cannot be changed
  from a script`; `update` на raw или urlencoded его заменяет.
- `pm.execution.skipRequest()` есть только в `Phase::PreRequest` (в test — `undefined`, как в Postman): текущий
  скрипт прекращается сразу и без ошибки, остальные pre-request не исполняются, `Report.skip_request = true`.
- `pm.execution.setNextRequest(x)` и `postman.setNextRequest(x)` ничего не делают.
- `postman.setEnvironmentVariable(k, v)` — как в Postman (сверено по `postman-sandbox` 6.9.0,
  `postman-legacy-interface.js`): значение приводится `v.toString()` только для истинных значений и для `false`;
  `0`, `''`, `null`, `undefined` идут как есть; затем `pm.environment.set`. `getEnvironmentVariable`,
  `clearEnvironmentVariable` (`unset`), `clearEnvironmentVariables` (`clear`); то же с `Global` для
  `pm.globals`; `postman.getResponseHeader(name)` — без учёта регистра, вне test — `undefined`.
- `postman.set* / clear*` меняют не только область, но и старый объект `environment` / `globals` **этого**
  скрипта (так в Postman): `set` кладёт в него `ключ = значение`, `clear…Variable(k)` удаляет ключ,
  `clear…Variables()` — все ключи. Прелюдия держит объект-снимок у себя (создаёт его при первом чтении
  глобального `environment` / `globals` либо при первом `postman.set* / clear*`); если скрипт заменил
  глобальное имя своим значением, меняется прежний объект прелюдии, значение скрипта не трогается.
- Старые снимки — ленивые свойства глобального объекта с сеттером (правило Task 3: их затеняет и `var data = …`,
  и присваивание без объявления `data = JSON.parse(responseBody)`):
  - `environment` и `globals` — `toObject()` области на момент создания снимка;
  - `request` — `{ id, name, method, url, headers, data }`: `headers` — объект включённых заголовков с ключами в
    **нижнем регистре**, повтор имени — массив значений (в Postman — `headers.toObject(true, false, true,
    true)`); `data` — для raw текст; для urlencoded объект `{ ключ: значение }` включённых пар; для form-data
    такой же объект текстовых полей (`ScriptBody::FormData`; файловые части в него не попадают); повтор ключа —
    массив значений; тела нет, `file`, `graphql` — `{}`. В test-скрипте `request` строится из test-запроса (см.
    правила `rqm-runner`): `url`, `headers`, `data` — с подставленными значениями; в pre-request — как введено;
  - `data` — `{}`; `iteration` — `0`.
- `ScriptBody::FormData` в `pm.request.body` ведёт себя как тело `Other`: `mode` — `formdata`,
  `toString()` — пустая строка, запись `raw` и правка `urlencoded` не применяются (`Warn` из правила выше),
  `update` на raw или urlencoded его заменяет.

**Правила `rqm-runner`:**
- `ScriptRequest` фазы pre-request — из `input.request`:
  - `url`: `request.url` делится по первому `#` на начало и фрагмент (модель хранит `#fragment` в конце `url`,
    а query — отдельными строками: `rqm-model/src/request.rs`); включённые строки `query` дописываются к началу
    как `ключ=значение` через `&` — после `?`, а если `?` в начале уже есть — после `&`; затем возвращается
    `#фрагмент`. Ключ и значение строки кодируются тем же `query_encode` (ключ — с `is_key = true`), но только
    **вне** `{{…}}`: кусок от `{{` до ближайшего `}}` (как его понимает `Resolver::substitute`) остаётся как
    набран. Так литеральные `&`, `#`, `=`, пробел в строке модели не ломают разбор адреса в скрипте и обратно, а
    переменная видна скрипту как `{{имя}}`.
  - `headers` — все строки (`disabled = !enabled`).
  - тело: `Raw` → `Raw`; `UrlEncoded` → `UrlEncoded` (все строки, `disabled = !enabled`); `None` и `Binary` без
    файла → `None`; `FormData` → `FormData` из включённых текстовых частей (ключ и значение как набраны);
    `Binary` → `Other("file")`; `GraphQl` → `Other("graphql")`.
- После pre-request строится запрос этой отправки — копия `input` (в хранилище и в черновик не пишется никогда):
  - метод — из отчёта;
  - адрес: строка отчёта равна той, что ушла в `Job`, → `request.url` и `request.query` не трогаются. Изменилась
    → разбирается обратно: фрагмент — после первого `#`; query — между первым `?` и фрагментом; `request.url` =
    часть до `?` плюс `#фрагмент`; query режется по `&` и по первому `=` в строки `Param { key, value, enabled:
    true, postman_extra: пусто }` (`{{…}}` при разрезании — неделимый кусок; пустые куски между `&`
    пропускаются; выключенные строки модели в копию не возвращаются). `request.query` **не** сливается в адрес:
    `prepare_with` по-прежнему подставляет переменные в каждую пару отдельно, `finish_url` кодирует их, а
    параметр авторизации (API key в query) по-прежнему заменяет одноимённую строку. Повторное кодирование
    безопасно: `query_encode` не трогает `%`, поэтому `a%26b` из адреса уйдёт как `a%26b`;
  - список заголовков изменился → строится заново из пар (`postman_extra` пуст);
  - тело: `Raw(text)` → `Body::Raw` с прежним языком, если тело было raw, иначе `RawLanguage::Text`; `UrlEncoded`
    → `Body::UrlEncoded`; `None` → `Body::None`; `FormData` и `Other` → прежнее тело.
  `prepare_with` получает эту копию.
- `prepare_with` дополнительно отдаёт `Prepared.script_body` — тело до отсечения по методу и до кодирования, с
  подставленными значениями: `Raw` → `Raw(подставленный текст)`; `UrlEncoded` → `UrlEncoded` из тех же
  подставленных пар, что кодируются в байты (`disabled = false`); `FormData` → `FormData` из подставленных
  текстовых частей; `Binary` с файлом → `Other("file")`; `GraphQl` → `Other("graphql")`; иначе `None`. Значения
  берутся из того же прохода подстановки, что и отправленные байты, — повторной подстановки нет
  (`{{$guid}}` и `{{$timestamp}}` у скрипта те же, что ушли в сеть).
- Фаза test получает **отправленный** запрос — как в Postman (`postman-runtime` 7.56.3, `item.command.js`:
  `ctxTemplate.request = request`). `ScriptRequest` фазы test: `method` и `url` — из `Prepared.http` (итоговый
  адрес с query, со схемой); `headers` — `http.headers` (свои включённые и заголовок авторизации, значения
  подставлены, `disabled = false`), после них выключенные строки копии запроса с `disabled = true`; `body` —
  `Prepared.script_body`. Отличие от Postman: автоматических заголовков (`User-Agent`, `Host`, `Content-Length`,
  `Accept-Encoding` …) в `pm.request.headers` нет (решение № 40). Этот `ScriptRequest` собирается **до**
  `engine.send`: движок забирает `HttpRequest` владением (так же, как сейчас `PreparedSummary::of`). Изменения
  запроса в test никуда не идут: `Report.request` и `Report.skip_request` фазы test не читаются.
- `skip_request` (после pre-request) → `Err(SendError::Skipped)`, `request: None`, test-скрипты не исполняются.

- [ ] **Step 1: Падающие тесты.**
  `crates/rqm-script/tests/request.rs` (запрос: `GET`, адрес `{{host}}/users/7?page=1&q=a%20b#frag`, заголовки
  `Accept: json` и отключённый `X-Off: 1`, тело `Raw("{\"a\":1}")`; `Info { request_id: "r-1", request_name:
  "Login" }`):
  - `reads` — `console.log(pm.request.method, pm.request.url.toString(), pm.request.url.getHost(),
    pm.request.url.getPath(), pm.request.url.getQueryString(), pm.request.url.getPathWithQuery(),
    pm.request.url.query.get('page'), pm.request.headers.get('accept'), pm.request.headers.has('X-Off'),
    pm.request.body.mode, pm.request.body.raw, pm.request.body.isEmpty())` → `GET
    {{host}}/users/7?page=1&q=a%20b#frag {{host}} /users/7 page=1&q=a%20b /users/7?page=1&q=a%20b 1 json false raw
    {"a":1} false`; для адреса `https://api.example.com:8443/a` — `getHost()` даёт `api.example.com`;
  - `mutations_come_back` — `PreRequest`: `pm.request.method = 'POST'; pm.request.url.query.add({ key: 'x', value:
    '1' }); pm.request.url.removeQueryParams('page'); pm.request.headers.add({ key: 'X-New', value: 'n' });
    pm.request.headers.upsert({ key: 'accept', value: 'xml' }); pm.request.removeHeader('X-Off');
    pm.request.addHeader('X-S: s'); pm.request.body.raw = '{"b":2}'` → `report.request`: `POST`, адрес
    `{{host}}/users/7?q=a%20b&x=1#frag`, заголовки `Accept: xml`, `X-New: n`, `X-S: s`, тело `Raw("{\"b\":2}")`;
  - `url_assignment_and_body_update` — `pm.request.url = 'http://other/p?z=1'; pm.request.body.update({ mode:
    'urlencoded', urlencoded: [{ key: 'a', value: '1' }] })` → адрес и `UrlEncoded([a=1])`; затем
    `pm.request.body.update('plain')` → `Raw("plain")`;
  - `file_modes_cannot_be_set` — `pm.request.body.update({ mode: 'file', file: { src: 'C:/x' } });
    pm.request.body.update({ mode: 'formdata', formdata: [] })` → тело прежнее, две строки `Warn`; при теле
    `FormData([a=1])`: `console.log(pm.request.body.mode, pm.request.body.toString() === '');
    pm.request.body.raw = 'x'` → `formdata true`, тело осталось `FormData([a=1])`, строка `Warn`; при теле
    `Other("file")` — `file true`, тело прежнее, строка `Warn`;
  - `changes_pass_to_the_next_script` — скрипт 1 добавляет заголовок, скрипт 2 печатает его значение;
  - `skip_request` — `PreRequest`, `["console.log('a'); pm.execution.skipRequest(); console.log('b')",
    "console.log('c')"]` → консоль `a`, `skip_request == true`, `errors` пуст; `Test`: `console.log(typeof
    pm.execution.skipRequest)` → `undefined`;
  - `test_phase_changes_nothing` — `Test`: `pm.request.method = 'DELETE'; pm.request.headers.add({ key: 'X', value:
    '1' }); pm.request.url = 'http://other'` → `errors` пуст, `report.request` равен запросу из `Job`,
    `skip_request == false`; модульный тест примитивов (`src/host.rs`): в `Phase::Test` примитивы изменения
    запроса и `skip_request` отвечают ошибкой и состояние не меняют;
  - `set_next_request_does_nothing` — `pm.execution.setNextRequest('x'); postman.setNextRequest(null);
    console.log('ok')` → `ok`, `errors` пуст;
  - `legacy_postman_object` — `postman.setEnvironmentVariable('n', 5); postman.setGlobalVariable('g', 'v');
    console.log(typeof postman.getEnvironmentVariable('n'), postman.getGlobalVariable('g'), environment.n,
    globals.g); postman.clearEnvironmentVariable('n'); console.log(environment.n);
    postman.clearGlobalVariables(); console.log(Object.keys(globals).length)` → `string v 5 v`, затем
    `undefined`, затем `0`; `changes` — `[Environment Set n "5", Globals Set g v, Environment Unset n, Globals
    Clear]`; `postman.setEnvironmentVariable('z', 0); postman.setEnvironmentVariable('f', false); console.log(
    typeof pm.environment.get('z'), typeof pm.environment.get('f'))` → `number string`; старый сценарий
    `postman.setEnvironmentVariable('token', 'T'); tests['saved'] = environment.token === 'T';` → тест `saved`
    пройден; `var environment = { mine: 1 }; postman.setEnvironmentVariable('q', '1'); console.log(
    environment.q, environment.mine)` → `undefined 1`; в `Test` `postman.getResponseHeader('content-type')` даёт
    значение заголовка;
  - `legacy_snapshots` — `console.log(environment.e, globals.g, request.method, request.url,
    request.headers.accept, request.headers.Accept, request.data, typeof data, iteration)` → `E G GET
    {{host}}/users/7?page=1&q=a%20b#frag json undefined {"a":1} object 0` (ключи заголовков — в нижнем регистре,
    как в Postman); заголовки `X-A: 1`, `x-a: 2` → `JSON.stringify(request.headers['x-a'])` даёт `["1","2"]`;
    тело `UrlEncoded([u=1, u=2, off=3 (выключена)])` → `JSON.stringify(request.data)` даёт `{"u":["1","2"]}`;
    тело `FormData([user=bob, ts=5])` → `request.data.user + request.data.ts` даёт `bob5`; тело `Other("file")`
    → `JSON.stringify(request.data)` даёт `{}`; `var data = 1; var request = 2; console.log(data + request)` →
    `3`; присваивание без объявления: `data = { a: 1 }; request = 'r'; environment = 'e'; console.log(data.a,
    request, environment)` → `1 r e`.
  `tests/tamper.rs`: оба теста дополнить `pm.request.*`, `postman.*`, `pm.execution.*`.
  `crates/rqm-runner/tests/scripts.rs`:
  - `pre_request_changes_the_request_that_is_sent` — pre: `pm.request.headers.add({ key: 'X-Sig', value: 'abc'
    }); pm.variables.set('stamp', '42'); pm.request.url.query.add({ key: 'ts', value: '{{stamp}}' });
    pm.request.body.raw = JSON.stringify({ n: 1 }); pm.request.method = 'PUT';` → сервер получил `PUT /x?ts=42` с
    `X-Sig: abc` и телом `{"n":1}`; `outcome.request` показывает то же; узел в хранилище не изменился;
  - `query_rows_of_the_model_reach_the_script` — адрес `{{host}}/x`, строки query `a=1` (включена) и `b=2`
    (выключена); pre: `console.log(pm.request.url.getQueryString())` → `a=1`; сервер получил `/x?a=1`;
  - `query_stands_before_the_fragment_and_stays_encoded` — адрес `{{host}}/x#frag`, строка query `q = {{v}}`,
    переменная `v` = `a&b c`; pre: `console.log(pm.request.url.getQueryString(), pm.request.url.toString().endsWith(
    '#frag')); pm.request.url.query.add({ key: 'x', value: '1' })` → консоль `q={{v}} true`; сервер получил
    `/x?q=a%26b%20c&x=1`;
  - `literal_query_characters_survive_a_script_edit` — строка query `k` со значением `a&b#c`, ключ второй строки
    `m=n` со значением `1`; pre: `console.log(pm.request.url.query.count()); pm.request.url.query.add({ key:
    'ts', value: '1' })` → консоль `2`; сервер получил `?k=a%26b%23c&m%3Dn=1&ts=1` — то же, что без скрипта,
    плюс `ts`;
  - `untouched_address_keeps_its_rows` — pre меняет только заголовок → запрос ушёл с тем же адресом и query, что
    и при выключенных скриптах (сравнение двух отправок);
  - `api_key_in_the_query_still_replaces_the_row` — auth коллекции «API key в query» с ключом `key`, у запроса
    строка query `key=old`; pre: `pm.request.url.query.add({ key: 'ts', value: '1' })` → сервер получил ровно
    один параметр `key` — значение из auth;
  - `test_script_sees_the_sent_request_and_cannot_change_it` — коллекция с bearer-auth `{{token}}` (`token` =
    `T-1`), адрес `{{host}}/x`, тело raw `{"n":{{id}}}` (`id` = `7`), выключенный заголовок `X-Off: 1`; pre
    ставит заголовок `X-P: 1`; test: `console.log(pm.request.headers.get('X-P'),
    pm.request.url.toString().startsWith('http://127.0.0.1'), JSON.parse(pm.request.body.raw).n,
    pm.request.headers.get('Authorization'), pm.request.body.mode, pm.request.headers.has('X-Off'),
    request.data, request.headers.authorization); pm.request.method = 'DELETE'` → консоль `1 true 7 Bearer T-1 raw
    false {"n":7} Bearer T-1`; сервер получил исходный метод;
  - `test_script_sees_the_sent_urlencoded_body` — тело urlencoded `a={{id}}`, `stamp={{$timestamp}}`; test:
    `console.log(pm.request.body.mode, pm.request.body.urlencoded.get('a'), pm.request.body.urlencoded.get(
    'stamp'))` → `urlencoded 7 <то же число, что получил сервер в поле stamp>`;
  - `pruned_body_is_still_visible_to_the_test` — метод `GET`, тело raw `{"n":{{id}}}` (отсекается по методу) →
    test: `console.log(pm.request.body.raw)` → `{"n":7}`;
  - `skip_request_sends_nothing` — `Err(SendError::Skipped)`, сервер не получил запросов, test-скрипты не
    исполнялись, консоль pre-request на месте;
  - `script_cannot_turn_the_body_into_a_file` — pre: `pm.request.body.update({ mode: 'file', file: { src:
    '<существующий временный файл>' } })` → сервер получил исходное тело raw.
- [ ] **Step 2: Запустить** `cargo test -p rqm-script -p rqm-runner` — новые тесты падают.
- [ ] **Step 3: Реализовать**; `npm run build` в `prelude/`.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: pm.request, старый объект postman, pm.execution.skipRequest».

---

### Task 9: Таблица API — `require`, библиотеки, заглушки, поиск неподдержанного

**Files:**
- Create: `crates/rqm-script/prelude/src/api.json`, `prelude/src/require.js`, `prelude/src/stubs.js`,
  `prelude/src/lodash-entry.js`, `prelude/built/lodash.js`, `crates/rqm-script/src/api.rs`
- Modify: `prelude/package.json` (+ `"lodash": "4.18.1"`), `package-lock.json`, `build.mjs` (третий вход →
  `built/lodash.js`), `prelude/src/index.js`, `prelude/src/response.js` (фабрика ответа зовёт `stubResponse`),
  `prelude/src/assertions.js` (три утверждения-заглушки), `built/prelude.js`, `built/chai.js` (пересборка: первая
  строка с контрольной суммой меняется); `crates/rqm-script/src/bundle.rs` (`Bundle::Lodash`), `src/host.rs` (имя
  `"lodash"` в списке примитива `load_bundle`), `src/lib.rs`
- Test: `crates/rqm-script/tests/api.rs`, `tests/engine.rs` (`no_ambient_io_globals`), `tests/bundle.rs`,
  `tests/tamper.rs`

**Interfaces:**
- Consumes: примитив `load_bundle(name)` (Task 5); фабрику объекта ответа (Task 3, 6) и плагин утверждений
  (Task 5); объект `pm.request` и его единственный на скрипт `pm.request.url` (Task 8); всё, что реализовали
  Task 2–8 (таблица их перечисляет).
- Produces:

```rust
/// The names of the Postman sandbox API that the text of a script uses and this engine does not support:
/// sorted, without repeats. A plain text search — comments and strings are searched too.
pub fn unsupported_api(script: &str) -> Vec<String>;
```

```json
// prelude/src/api.json — единственный список API: его читают прелюдия (заглушки), Rust (поиск) и тест окна (Task 16)
{ "members": [ { "name": "pm.environment.get", "kind": "function", "status": "v1", "hint": "get(key)" },
               { "name": "pm.response.code", "kind": "property", "status": "v1", "phase": "test" },
               { "name": "pm.execution.setNextRequest", "kind": "function", "status": "noop" },
               { "name": "pm.vault", "kind": "property", "status": "unsupported" } ],
  "modules": [ { "name": "lodash", "status": "v1" }, { "name": "moment", "status": "unsupported" } ] }
```

**Правила:**
- `status`: `v1` — работает; `noop` — есть, но ничего не делает; `unsupported` — «позже». `kind`: `function` или
  `property`. `phase` (необязательно): `prerequest` или `test` — имя есть только в этой фазе. `hint`
  (необязательно) — короткая подпись для подсказки, на английском.
- Содержимое — раздел «API в первой версии» этого плана, по одному элементу на конечное имя (у четырёх областей
  переменных — по семь методов каждая; у `pm.response.to` — каждое утверждение как `pm.response.to.have.status`,
  `pm.response.to.be.ok` и т. д.; `pm.request.to` — один элемент `pm.request.to`, `property`, `unsupported`;
  старые глобальные имена — как есть: `responseBody`, `tests`, `postman.setEnvironmentVariable`; глобальные
  библиотеки «позже» — `CryptoJS`, `tv4`, `cheerio`, `Backbone`, `Buffer`, `xml2Json`, `responseCookies`).
  `modules` — все модули из строк `require` таблицы.
- Два правила формы таблицы (их проверяет `table_is_well_formed`): (1) у элемента `unsupported` потомков в таблице
  нет — из строки «`pm.cookies`, `pm.cookies.jar()`» остаётся один `pm.cookies` (`property`), а `pm.cookies.jar()`
  покрывает правило самого длинного префикса; (2) имя таблицы может быть собственным префиксом другого имени
  только при одинаковом статусе (`pm.test` и `pm.test.skip` — оба `v1`).
- **Пространство имён** — любой собственный префикс (по границе сегмента) любого имени таблицы: `pm.response`,
  `pm.response.to`, `pm.response.to.have`, `pm.request.url`, `pm.environment`. Отдельными элементами
  пространства имён в таблицу не вносятся — они выводятся из имён.
- Заглушки неподдержанного ставятся в трёх местах; текст везде один — `Error("<имя> is not supported in
  RequestMaster")`; все свойства-заглушки — `enumerable: false`, `configurable: true` (печать объекта и
  `Object.keys` их не задевают):
  1. `stubs.js` экспортирует `installStubs(phase)`. `index.js` зовёт её **после** сборки `pm.request`,
     `pm.response` и остальных объектов. Она обходит элементы `unsupported`, кроме имён, начинающихся с
     `pm.response.`: элемент с `phase` другой фазы пропускается; недостающие объекты-родители создаются (так
     появляются `pm.vault`, `pm.cookies`, `pm.datasets`, `pm.message`); `function` → функция, которая бросает;
     `property` → геттер, который бросает. Члены `pm.request` (`auth`, `id`, `name`, `size`, `toJSON`, `clone`,
     `to`) и `pm.request.url` (`variables`, `protocol`, `host`, `port`, `path`, `hash`) ставятся на уже
     существующие объекты.
  2. `stubs.js` экспортирует `stubResponse(obj)` — заглушки членов объекта ответа (элементы `pm.response.<имя>`
     без `to.`: `cookies`, `size`, `jsonp`, `dataURI`, `mime`, `contentInfo`, `encoding`, `originalRequest`,
     `timingPhases`). Её зовёт фабрика ответа в `response.js` на каждый созданный объект — и для `pm.response`, и
     для ответа `pm.sendRequest`. В pre-request `pm.response` поэтому по-прежнему не существует.
  3. `pm.response.to.have.jsonSchema`, `responseTime`, `responseSize` — три метода в плагине `assertions.js`
     (`addMethod`; тело бросает обычный `Error`, не `AssertionError`). Они регистрируются вместе с chai, лениво:
     `stubs.js` chai не грузит, `api.json` в bundle chai не входит — имена записаны в плагине, а тест сверяет их
     с таблицей. (Без этих методов chai 4 бросал бы своё `Invalid Chai property: jsonSchema`.)
- `noop`-функции возвращают `undefined`; `pm.iterationData`: `get` → `undefined`, `has` → `false`, `toObject` →
  `{}`.
- `require(name)`: `lodash` → лениво грузит `built/lodash.js` (`load_bundle("lodash")`) и отдаёт lodash; `chai` →
  chai (лениво, тот же объект, что за `pm.expect`); `atob`, `btoa` → одноимённые функции движка; модуль со
  статусом `unsupported` и любой неизвестный → `Error: Cannot find module '<имя>' (not supported in
  RequestMaster)`. Глобальные `atob` и `btoa` — те, что уже есть в QuickJS-NG; прелюдия их не переопределяет.
- Глобальный `_` — ленивое свойство с сеттером (правило Task 3): lodash 4.18.1 (в Postman здесь lodash 3 —
  отличие названо в таблице API).
- `unsupported_api(text)` находит три вещи.
  (1) Цепочки `pm` и `postman`: само имя и идущие подряд сегменты `.имя`; цепочка обрывается на первом символе,
  который не точка и не часть идентификатора (`(`, `[`, пробел, `;` …). В цепочке, начинающейся с
  `pm.response.to`, сегменты `not` перед сопоставлением выбрасываются (и в ответ имя идёт без `not`); больше
  ничего не выбрасывается — `to`, `be`, `have` входят в имена таблицы. Дальше по порядку:
  - (а) берётся самый длинный префикс цепочки, равный элементу таблицы: статус `unsupported` → в ответ идёт имя
    этого элемента; `v1` или `noop` → ничего;
  - (б) такого префикса нет, а вся цепочка — пространство имён (`pm.response`, `pm.request.url = 'x'`,
    `pm.expect(pm.response)`, `const e = pm.environment`) → ничего;
  - (в) такого префикса нет, а первые три сегмента цепочки — `pm.response.to` → ничего: это свободная цепочка chai
    (`pm.response.to.be.an('object')`, `pm.response.to.have.property('code')`);
  - (г) иначе в ответ идёт самый короткий префикс цепочки, который не является ни именем таблицы, ни
    пространством имён: `pm.foo.bar()` → `pm.foo`; `pm.response.foo()` → `pm.response.foo`.
  (2) `require('x')` и `require("x")` с модулем `unsupported` или неизвестным → `require('x')`.
  (3) Отдельные идентификаторы, равные глобальному имени со статусом `unsupported`, перед которыми не стоит
  точка.

- [ ] **Step 1: Падающие тесты** (`tests/api.rs`):
  - `table_is_well_formed` — файл разбирается; имена не повторяются; статусы и виды — из допустимых; ни одно имя
    не начинается с `<имя unsupported-элемента>.`; имя, которое является собственным префиксом другого имени,
    имеет тот же статус; точечные проверки: `pm.collectionVariables.set` — `v1`; `responseBody` — `v1`, `phase:
    test`; `pm.execution.skipRequest` — `v1`, `phase: prerequest`; `pm.execution.setNextRequest` — `noop`;
    `pm.vault`, `pm.cookies`, `pm.request.to`, `pm.response.to.have.jsonSchema` — `unsupported`; элемента
    `pm.cookies.jar` нет; модули `lodash` — `v1`, `moment` — `unsupported`;
  - `every_v1_and_noop_member_exists_in_the_engine` — для каждого элемента `v1` / `noop`: в `Job` его фазы (без
    `phase` — в `Test`; у `Job` заданы имя окружения, запрос и ответ 200). Правило обхода: скрипт идёт по
    сегментам имени от `globalThis` и **читает** все сегменты, кроме последнего, бросая ошибку на `undefined` /
    `null`; последний сегмент не читается, а проверяется наличием — `last in parent`; только у элементов `kind:
    "function"` значение дополнительно читается и проверяется `typeof === "function"` → `errors` пуст. Причина:
    свойства-утверждения chai (`pm.response.to.be.notFound`, `clientError`, `error`, `accepted` …) исполняют
    проверку при одном чтении и на ответе 200 бросили бы `AssertionError`; оператор `in` видит геттеры
    `Assertion.prototype` и аксессоры `pm.request.body`, не вызывая их, и проходит сквозь Proxy chai (у него
    только ловушка `get` — проверено опытом); промежуточные `to`, `be`, `have` — цепочки без проверки. Чтение
    методов (`pm.response.to.have.status`) проверку не запускает. Здесь проверяется только наличие имён —
    поведение утверждений ответа проверяют тесты Task 5;
  - `every_unsupported_member_throws_its_name` — функция при вызове, свойство при чтении бросают `Error: <имя> is
    not supported in RequestMaster`; элемент с `phase` проверяется в своей фазе, остальные — в обеих;
    `pm.response.to.have.jsonSchema({})` бросает именно `Error: pm.response.to.have.jsonSchema is not supported in
    RequestMaster` (не `AssertionError` и не `Invalid Chai property`), то же через `to.not.have.jsonSchema({})`;
    три имени-заглушки плагина совпадают с элементами таблицы `pm.response.to.have.*` со статусом `unsupported`;
  - `stubs_keep_phases_apart` — `PreRequest`: `console.log(typeof pm.response, typeof responseBody)` →
    `undefined undefined` (заглушки не создали `pm.response`);
  - `sendrequest_response_has_stubs` — `pm.sendRequest('http://x/a', (e, res) => { try { res.cookies } catch (err)
    { console.log(err.message) } })` → `pm.response.cookies is not supported in RequestMaster`;
  - `stubs_do_not_break_printing` — `Test`: `console.log(pm.response); console.log(Object.keys(pm.response).length
    >= 0, Object.keys(pm.request).includes('auth'))` → `errors` пуст, вторая строка `true false`;
  - `stubs_survive_url_assignment` — `PreRequest`: `pm.request.url = 'http://other/p'; try { pm.request.url.host }
    catch (err) { console.log(err.message) }` → `pm.request.url.host is not supported in RequestMaster`;
  - `require_modules` — `const l = require('lodash'); console.log(l.VERSION, l.get({ a: [{ b: 1 }] }, 'a[0].b'),
    _.chunk([1, 2, 3], 2).length, require('chai').expect === pm.expect, require('atob')('YQ=='), btoa('a'))` →
    `4.18.1 1 2 true a YQ==`; `require('moment')` → `Exception`, `Error: Cannot find module 'moment' (not
    supported in RequestMaster)`; `require('fs')` → то же с `fs`;
  - `scanner` — `pm.environment.set('a', 1); pm.response.json().data.items; pm.expect(pm.response.code).to.equal(
    200); require("lodash")` → пусто; `pm.vault.get('x'); const m = require('moment'); CryptoJS.MD5('a');
    pm.foo.bar(); postman.getResponseCookie('a'); x.Buffer; pm.vault.set('y', 1)` → `["CryptoJS", "pm.foo",
    "pm.vault", "postman.getResponseCookie", "require('moment')"]`; `// pm.vault` → `["pm.vault"]`; пустой текст
    → пусто; пространства имён и отрицание: `pm.expect(pm.response).to.have.status(200); const r = pm.response;
    const e = pm.environment; pm.request.url = 'x'; pm.response.to.not.be.error; pm.response.to.not.have.status(
    404); pm.response.to.not.be.ok; pm.response.to.be.an('object'); pm.response.to.have.property('code')` →
    пусто; `pm.response.to.not.have.jsonSchema(s); pm.response.foo(); pm.request.to.have.header('a');
    pm.cookies.jar()` → `["pm.cookies", "pm.request.to", "pm.response.foo", "pm.response.to.have.jsonSchema"]`.
  `tests/engine.rs::no_ambient_io_globals`: `require` из списка убрать, добавить проверку, что `require('fs')`
  бросает. `tests/bundle.rs` — сумма сходится и у `built/lodash.js`, и у пересобранного `built/chai.js`.
  `tests/tamper.rs`: оба теста дополнить `require('lodash')`, `_`, чтением заглушек.
- [ ] **Step 2: Запустить** `cargo test -p rqm-script` — новые тесты падают.
- [ ] **Step 3: Реализовать**; в `prelude/`: `npm install`, `npm run build`. В отчёт — размеры трёх файлов
  `built/`.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: таблица API, require и lodash, заглушки неподдержанного, поиск по тексту».

---

### Task 10: Ядро — запись переменных, согласие, настройки скриптов

**Files:**
- Create: `crates/rqm-runner/src/variable_changes.rs`, `crates/rqm-runner/src/trust.rs`
- Modify: `crates/rqm-runner/src/lib.rs`, `src/send.rs`; `src-tauri/src/settings.rs` (в `From<SettingsDto> for
  AppSettings` — `..AppSettings::default()`, до Task 12)
- Test: `crates/rqm-runner/tests/variable_changes.rs`, `tests/trust.rs` (новые), `tests/scripts.rs`;
  `src-tauri/tests/core_session.rs` — только чтобы тесты оболочки собирались: два полных литерала `AppSettings {
  timeout, verify_tls, proxy }` (сейчас строки 385 и 461) дополняются `..AppSettings::default()`. Других полных
  литералов `AppSettings` в репозитории нет: в `crates/rqm-runner/tests/send.rs` уже стоит
  `..Default::default()`, в `tests/owner_end_to_end.rs` литералов нет — эти два файла задача не трогает

**Interfaces:**
- Consumes: `Store::{globals, environments, save_environment, node, ancestors, update_content, state_get,
  state_set}`; `rqm_model::{Variable { key, value, secret, enabled, postman_extra }, Environment, NodeContent,
  Extra}`; `VariableChange`, `VariableOp`, `StoredScope` (Task 2); образец «функция runner'а пишет в Store» —
  `cookies_state.rs`.
- Produces:

```rust
// crates/rqm-runner/src/lib.rs
pub struct AppSettings { /* timeout, verify_tls, proxy — как раньше */
                         /// Whether scripts run at all.
                         pub scripts_enabled: bool,
                         /// JavaScript execution time of one script.
                         pub script_timeout: Duration }
// Default: scripts_enabled = true, script_timeout = 10 s
// load_send_input (подпись прежняя) считает SendInput::scripts:
//   !settings.scripts_enabled                         -> Off(ScriptsOff::Disabled)
//   !scripts_allowed(store, <id коллекции запроса>)   -> Off(ScriptsOff::NotAllowed { collection: <её имя> })
//   иначе                                             -> Run

// crates/rqm-runner/src/trust.rs, re-exported
/// Key of `app_state`: a JSON array with the ids of the collections whose scripts may run.
pub const SCRIPTS_ALLOWED_KEY: &str = "scripts_allowed";
/// The id of the collection of the node: the node itself or its first ancestor. Unknown node — `NotFound`.
pub fn collection_of(store: &Store, node_id: &str) -> Result<String, StoreError>;
pub fn scripts_allowed(store: &Store, collection_id: &str) -> Result<bool, StoreError>;
pub fn set_scripts_allowed(store: &mut Store, collection_id: &str, allowed: bool) -> Result<(), StoreError>;
/// Drops the ids of the collections that are no longer in the tree.
pub fn forget_deleted_collections(store: &mut Store) -> Result<(), StoreError>;

// crates/rqm-runner/src/variable_changes.rs, re-exported
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedChange { pub scope: StoredScope,
                           /// `"globals"`, the id of the environment or the id of the collection node.
                           pub target_id: String, pub op: VariableOp }
/// The one rule of a script change on a stored list of variables.
pub fn apply_to_variables(variables: &mut Vec<Variable>, op: &VariableOp);
/// Applies the changes over what the store holds now and returns what was applied, in the order of `changes`.
pub fn apply_variable_changes(store: &mut Store, collection_id: &str, environment_id: Option<&str>,
                              changes: &[VariableChange]) -> Result<Vec<AppliedChange>, StoreError>;
```

**Правила:**
- `apply_to_variables`: `Set { key, value }` — у **последней включённой** переменной с этим именем меняется
  только `value` (`secret`, `enabled`, `postman_extra` не трогаются); такой нет (или есть только выключенные) —
  в конец добавляется `Variable { key, value, secret: false, enabled: true, postman_extra: Extra::new() }`.
  `Unset { key }` — удаляются все включённые переменные с именем. `Clear` — удаляются все включённые. Выключенные
  строки не меняются никогда.
- `apply_variable_changes`: для каждой области, у которой есть изменения, — свежее чтение (`store.globals()`;
  окружение по id среди `store.environments()`; узел коллекции `store.node(collection_id)`), операции по порядку,
  запись — только если список изменился (`save_environment` / `update_content`; у узла меняется только
  `variables`, верхний `postman_extra` остаётся из прочитанного). Снимок, взятый в начале отправки, не пишется.
- `environment_id == None` — изменения окружения пропускаются. Окружение или коллекция к этому моменту удалены —
  их изменения пропускаются, это не ошибка; окружение **не создаётся заново**. Строки глобальных может не быть —
  она создаётся.
- Возвращаются операции существующих целей; пропущенные — нет.
- Общей транзакции на три области нет: вызывающий держит замок хранилища на всё время вызова. Отказ хранилища —
  `Err`; области, записанные до него, остаются записанными.
- Значение `scripts_allowed`, которое не читается как массив строк, — пустой список. Список без повторов.
- `send`: `Limits { time: input.settings.script_timeout, ..Limits::default() }`.

- [ ] **Step 1: Падающие тесты.**
  `tests/variable_changes.rs`:
  - `one_rule_for_set_unset_clear` — исходный список: `a=1`; `a=2` выключена; `b=3` секретная, с `postman_extra`
    `{"id":"x","type":"secret"}`; `a=4`. `Set a 9` → меняется только четвёртая; `Set b t` → значение `t`,
    `secret` и `postman_extra` прежние; `Set n v` → в конце новая включённая, не секретная, с пустым
    `postman_extra`; список из одной выключенной `d=0` и `Set d 1` → выключенная на месте, добавлена включённая
    `d=1`; `Unset a` на исходном → остались `a=2` (выключена) и `b`; `Clear` на исходном → осталась только
    `a=2`. (Эту же таблицу повторяет тест окна в Task 13.)
  - `changes_are_applied_over_a_fresh_read` — окружение `Dev`: `x=1`; после чтения входа отправки «другой
    писатель» сохраняет `x=1, y=user`; `apply_variable_changes(.., [Environment Set x 2])` → в базе `x=2, y=user`;
  - `three_scopes_are_written_and_reported` — `[Globals Set g 1, Collection Set token T, Environment Unset x,
    Environment Set z 3]` → значения в базе; у узла коллекции `auth`, `scripts` и верхний `postman_extra` прежние;
    ответ — четыре `AppliedChange` в том же порядке с `target_id` `"globals"`, id коллекции, id окружения;
  - `globals_row_is_created_when_missing`;
  - `missing_targets_are_skipped` — `environment_id: None` → изменения окружения не применены и не возвращены;
    окружение удалено до вызова → пропущено и **не появилось** в `store.environments()`; коллекция удалена →
    пропущено, `Ok`;
  - `unchanged_list_is_not_written` — `Unset nope` → содержимое строки в базе байт в байт прежнее;
  - `export_keeps_untouched_rows` — окружение импортировано из JSON, где у переменной `n` значение числом `5` и
    есть ключ `description`; `Set other v` → в экспорте `n` по-прежнему число `5` с `description`, `other` —
    новая строка;
  - `export_of_a_collection_keeps_untouched_variable_rows` — коллекция импортирована из JSON (`import_collection`),
    где у переменной `n` значение числом `5` и есть ключи `description`, `id`, у переменной `token` — `value:
    "old"` и `type: "string"`, а у самой коллекции есть `info`, `event` (скрипт) и `auth`; `[Collection Set token
    T]` → в `export_collection`: строка `n` дословно прежняя (число `5`, `description`, `id`); у `token` изменился
    только `value` (`type` на месте); `info`, `event`, `auth` и остальные ключи верхнего уровня равны исходным
    (сравнение `serde_json::Value` экспорта до и после: отличается ровно один путь — `variable[token].value`).
  `tests/trust.rs`: `nothing_is_allowed_by_default`; `allow_and_revoke` (значение в `app_state` — JSON-массив с
  одним id, после отзыва — пустой массив); `unreadable_value_counts_as_empty` (значения `"abc"`, `{}`, `[1]`);
  `forget_deleted_collections_drops_missing_ids`; `collection_of_a_request_a_folder_and_a_collection` (неизвестный
  узел — `NotFound`); `load_send_input_decides_the_mode` (не разрешена → `Off(NotAllowed { collection: "Col" })`;
  разрешена → `Run`; разрешена и `scripts_enabled: false` → `Off(Disabled)`).
  `tests/scripts.rs`: `script_timeout_comes_from_the_settings` — `script_timeout` 200 мс, pre `while (true) {}` →
  `Err(SendError::Script)` с `kind == Timeout`;
  `native_loop_in_pre_request_ends_the_send` — `script_timeout` 200 мс, pre `pm.globals.set('a', '1');
  [].reverse.call({ length: 2 ** 53 - 1 })` → `Err(SendError::Script)` с `kind == Timeout` не позже чем через
  200 мс + `WATCHDOG_GRACE` + 1 с; в `outcome.scripts.changes` есть `Globals Set a 1`; сервер не получил
  запросов; следующая отправка этого же теста (без скриптов) проходит. Это единственный тест файла, который
  оставляет брошенный поток до конца процесса теста;
  `host_memory_limit_ends_the_send_and_keeps_the_store_small` — pre `try { while (true) pm.globals.set('k',
  'x'.repeat(1 << 19)) } catch (e) {}` → `Err(SendError::Script)` с `kind == Memory`; после
  `apply_variable_changes(.., &outcome.scripts.changes)` значение `k` в хранилище не длиннее `MAX_VALUE_BYTES`,
  строка глобальных — меньше 2 МиБ.
- [ ] **Step 2: Запустить** `cargo test -p rqm-runner` — новые тесты падают.
- [ ] **Step 3: Реализовать.** В тестах оболочки правятся только два литерала `AppSettings` в `core_session.rs`
  (`..AppSettings::default()`); поведение и остальные проверки прежние: ни одна коллекция ещё не разрешена,
  предупреждение приходит прежнего вида (временное сопоставление Task 4).
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: запись изменений переменных в хранилище, согласие на скрипты, предел времени».

---

### Task 11: Оболочка — результат отправки, история, согласие; снятие «скрипты не выполняются»

С этой задачи скрипты исполняются в приложении (для разрешённых коллекций), поэтому она же убирает надпись и
тексты «не выполняются».

**Files:**
- Create: `src-tauri/src/scripts.rs`
- Modify: `src-tauri/src/dto.rs`, `send.rs`, `history.rs`, `core.rs`, `commands.rs`, `lib.rs`, `main.rs`,
  `build.rs`, `capabilities/default.json`
- Modify: `ui/src/lib/types.ts`, `api.ts`, `testing/fakeApi.ts`, `sending.svelte.ts`, `strings.ts`,
  `ui/src/components/ScriptsEditor.svelte`, `SendWarnings.svelte`
- Test: `src-tauri/tests/core_send.rs`, `core_history.rs`, `core_data.rs`, `core_tree.rs`, `core_session.rs`;
  окно — новые случаи в `ui/src/components/RequestEditor.test.ts`, `ResponsePanel.test.ts`, `History.test.ts`,
  `ui/src/lib/sending.test.ts`; и правка литералов (новые обязательные ключи): `SendResult` собран литералом в
  `ui/src/App.test.ts`, `components/History.test.ts`, `components/RichMotion.test.ts`,
  `components/ResponsePanel.test.ts`, `lib/sending.test.ts`; запись истории
  (`tests`, `script_errors`) — в `ui/src/App.test.ts`, `lib/Session.test.ts`, `components/History.test.ts`;
  значения по умолчанию — в `lib/testing/fakeApi.ts`. Список сверить поиском по `warnings:` и по типам записи
  истории перед началом — `npm run check` назовёт пропущенное

**Interfaces:**
- Consumes: `SendOutcome.scripts: ScriptsReport`, `ScriptSource`, `TestOutcome`, `ConsoleEntry`, `ScriptFailure`,
  `SendError::{Script, Skipped, Cancelled}`, `SendWarning::ScriptsNotRun`, `ScriptsOff` (Task 4, 8);
  `apply_variable_changes`, `AppliedChange`, `collection_of`, `scripts_allowed`, `set_scripts_allowed`,
  `forget_deleted_collections` (Task 10).
- Produces:

```rust
// src-tauri/src/dto.rs — формы JSON из раздела «Контракт команд»; все перечисления #[serde(rename_all = "snake_case")]
pub struct ScriptSourceDto { pub node_id: String, pub node_name: String, pub node_kind: NodeKind, pub phase: ScriptPhaseDto }
pub enum ScriptPhaseDto { PreRequest, Test }
pub struct TestResultDto { pub source: ScriptSourceDto, pub name: String, pub passed: bool, pub skipped: bool, pub error: Option<String> }
pub struct ScriptErrorDto { pub source: ScriptSourceDto, pub kind: ScriptErrorKindDto, pub message: String,
                            pub line: Option<u32>, pub column: Option<u32> }
pub enum ScriptErrorKindDto { Exception, Timeout, Memory, Cancelled, Internal }
pub struct ConsoleLineDto { pub source: Option<ScriptSourceDto>, pub level: ConsoleLevelDto, pub text: String, pub at_ms: u64 }
pub enum ConsoleLevelDto { Log, Info, Warn, Error, Debug, Network }
pub struct VariableChangeDto { pub scope: VariableScope, pub target_id: String, pub op: VariableOpDto,
                               pub key: String, pub value: Option<String> }
pub enum VariableOpDto { Set, Unset, Clear }
pub struct SendResult { /* response, error, warnings */ pub tests: Vec<TestResultDto>, pub console: Vec<ConsoleLineDto>,
                        pub console_truncated: bool, pub script_errors: Vec<ScriptErrorDto>,
                        pub variable_changes: Vec<VariableChangeDto> }
pub enum SendErrorKind { /* … */ Script, Skipped }
pub enum WarningKind { /// Only in history entries written before scripts could run.
                       ScriptsNotExecuted, /* … */ ScriptsDisabled, ScriptsNotAllowed, VariablesNotSaved }
pub struct HistoryDetail { /* … */ #[serde(default)] pub tests: Vec<TestResultDto>,
                           #[serde(default)] pub script_errors: Vec<ScriptErrorDto> }   // derive Eq остаётся
// src-tauri/src/scripts.rs
impl AppCore { pub fn scripts_allowed(&self, node_id: &str) -> Result<bool, String>;
               pub fn set_scripts_allowed(&self, node_id: &str, allowed: bool) -> Result<(), String>; }
```

```ts
// ui/src/lib/types.ts — ScriptSource, TestResult, ConsoleLine, ScriptError, VariableChange и новые ключи SendResult,
// HistoryEntry (tests, script_errors), значения SendErrorKind и Warning["kind"] — как в «Контракте команд»
// ui/src/lib/api.ts
scriptsAllowed(nodeId: string): Promise<boolean>;                    // invoke("scripts_allowed", { node_id })
setScriptsAllowed(nodeId: string, allowed: boolean): Promise<void>;  // invoke("set_scripts_allowed", { node_id, allowed })
// ui/src/lib/testing/fakeApi.ts
// FakeSeed.scriptsAllowed?: string[]  — id разрешённых коллекций; FakeApi.allowCalls: [string, boolean][]
```

**Правила оболочки:**
- `send_request` после `rqm_runner::send`, до возврата, по порядку: (1) если `outcome.scripts.changes` не пуст —
  под одним замком `apply_variable_changes(&mut store, <id коллекции = input.ancestors[0].id>, <id окружения
  входа>, &changes)`: `Ok(applied)` → `variable_changes`; `Err(e)` → `variable_changes` пуст и предупреждение
  `VariablesNotSaved` с `items: [текст ошибки]` — ответ пользователю не теряется; (2) `record_history`; (3)
  `save_cookies`; (4) `SendResult`.
- **Дельта не теряется** (решение № 41). Сейчас команда после отправки может вернуть `Err` в двух местах:
  `response_dto` не прочитал файл тела; `save_cookies` отказал после неудачной отправки (`cookies_saved?`). Если
  к этому моменту `variable_changes` не пуст, команда вместо `Err` отвечает `SendResult` с этой дельтой, тестами,
  консолью и ошибками скриптов: в первом случае `response: None`, `error: { kind: Io, message: <текст отказа> }`
  (файл тела удаляется, как сейчас); во втором — исходная ошибка отправки, отказ cookies игнорируется (как уже
  делается в ветке ответа). Когда `variable_changes` пуст, оба места ведут себя как раньше — существующие тесты
  не меняются.
- `ScriptSourceDto.node_name` обрезается до 200 символов по границе символа (`chars().take(200)`): имя узла
  повторяется в каждом тесте и не должно раздувать запись истории и ответ команды.
- Сопоставления: `ScriptsNotRun { reason: Disabled, .. }` → `ScriptsDisabled`, `items: []`; `ScriptsNotRun {
  reason: NotAllowed { collection }, .. }` → `ScriptsNotAllowed`, `items: [collection]`; `SendError::Script(f)` →
  `kind: Script`, `message: f.message`; `Skipped` → `kind: Skipped`, `message: ""`; `Cancelled` → `Cancelled`.
  Временные сопоставления Task 4 и Task 8 (`ScriptsNotExecuted`, `Other`) удаляются. `VariableChangeDto`: `Set {
  key, value }` → `op: Set`, `value: Some`; `Unset { key }` → `value: None`; `Clear` → `key: ""`, `value: None`;
  `scope`: `Globals` → `Global`, `Collection`, `Environment`.
- История: `detail.tests` и `detail.script_errors` — из отчёта; консоль не пишется; `error_message` при
  `SendError::Script` — текст ошибки скрипта. В `summary.error_kind` при этом попадают новые значения `script` и
  `skipped` (раздел «Изменения в базе данных»); старые записи не меняются.
- Согласие: `scripts_allowed(node_id)` и `set_scripts_allowed(node_id, allowed)` находят коллекцию через
  `collection_of`. `create_node` для `NodeKind::Collection` сразу разрешает новую коллекцию. `duplicate_node`:
  исходник — разрешённая коллекция → копия разрешена. `delete_node` после удаления вызывает
  `forget_deleted_collections`. `import_files` согласия не даёт.
- Две команды — в четырёх местах; число команд 39.

**Правила окна (только синхронизация типов и снятие надписи; новые экраны — Task 13–15):**
- `sending.svelte.ts`: `cancelledResult()` получает пустые новые ключи.
- `strings.ts`: удалить `scripts.note`. `response.warnings`: `scripts_not_executed` (остаётся только для старых
  записей истории) — `` (nodes) => `Скрипты не выполнялись (${nodes.join(", ")}): отправка сделана до появления
  выполнения скриптов` ``; новые ключи — тоже **функции** (`SendWarnings.svelte` вызывает каждый элемент
  `response.warnings` как функцию от `items`; обычная строка дала бы ошибку типов или `TypeError` при показе):
  `scripts_disabled` — `` () => "Скрипты не выполнены: выполнение скриптов выключено в настройках" ``;
  `scripts_not_allowed` — `` ([collection]) => `Скрипты не выполнены: для коллекции «${collection}»
  они не разрешены` ``; `variables_not_saved` — `` ([reason]) => `Переменные, изменённые скриптом, не сохранены:
  ${reason}` ``. `response.errors`: `script` — «Ошибка в pre-request скрипте — запрос не отправлен», `skipped` —
  «Запрос пропущен скриптом (pm.execution.skipRequest)». `history.errors`: `script` — «ошибка скрипта», `skipped`
  — «пропущен».
- `ScriptsEditor.svelte`: удалить `<span class="note">`, правило `.note` и комментарий «They are only kept:
  nothing runs them yet».
- `SendWarnings.svelte`: класс `strong` — у `scripts_disabled`, `scripts_not_allowed`, `variables_not_saved`;
  комментарий исправить.
- Мёртвый код — поиск по всему репозиторию перед коммитом: `scripts.note`, `появится на следующем этапе`,
  `nothing runs them yet`, `ScriptsNotExecuted` (должен остаться только вариант `WarningKind` в `dto.rs`, его
  строка в `strings.ts`, значение в `types.ts` и тесты чтения старой истории), временные ветки `SendErrorKind::
  Other` для скриптов.

- [ ] **Step 1: Падающие тесты.**
  `core_send.rs` (wiremock; коллекция создаётся `create_node` — она разрешена; содержимое — `save_node_content`):
  - `scripts_run_and_their_results_come_back` — pre запроса: `pm.environment.set('stage', 'pre');
    console.log('hi')`; test: `pm.test('ok', () => {}); pm.collectionVariables.set('token', JSON.parse(
    responseBody).t)`; окружение выбрано → `tests` — один пройденный с `source.node_kind == "request"`,
    `source.phase == "test"`; в `console` есть `hi` (`source.phase == "pre_request"`) и строка `network` с
    `source == null`; `variable_changes` — `[{"scope":"environment","target_id":"<id окружения>","op":"set",
    "key":"stage","value":"pre"}, {"scope":"collection","target_id":"<id коллекции>","op":"set","key":"token",
    "value":"T"}]`; `get_environment` и `get_node` коллекции показывают новые значения; набор ключей JSON
    результата — ровно как в «Контракте команд»;
  - `imported_collection_does_not_run_scripts_until_allowed` — импорт коллекции, чей test-скрипт ставит
    глобальную переменную; Send → `warnings[0] == {"kind":"scripts_not_allowed","items":["<имя коллекции>"]}`,
    глобальные прежние, `tests` пуст; `set_scripts_allowed(<id запроса>, true)`; Send → переменная записана;
    `scripts_allowed(<id коллекции>) == true`; `set_scripts_allowed(.., false)` → снова предупреждение;
  - `pre_request_error_is_a_script_error` — `error.kind == "script"`, `error.message == "Error: nope"`,
    `response == null`, `script_errors[0].line == 2`; запись истории: `error_kind == "script"`;
  - `skip_request_is_reported` — `error.kind == "skipped"`;
  - `changes_are_saved_when_the_send_fails_or_is_cancelled` — pre ставит глобальную, адрес — закрытый порт →
    `error.kind == "connect"`, переменная в базе и в `variable_changes`; pre ставит глобальную и ждёт `await new
    Promise((r) => setTimeout(r, 30000))`, `cancel_send` через 100 мс → `error.kind == "cancelled"`, переменная в
    базе;
  - `edits_made_during_the_send_are_not_lost` — сервер отвечает через 300 мс; пока отправка идёт,
    `save_environment(env, [a=user])`; test-скрипт ставит `b` → в окружении и `a=user`, и `b`;
  - `storage_failure_while_saving_variables_is_a_warning` — пока отправка идёт, таблица `environment`
    переименована вторым соединением `rusqlite` (приём теста `broken_history_does_not_take_the_response_away`) →
    ответ получен, `variable_changes` пуст, в `warnings` есть `variables_not_saved`;
  - `script_changes_come_back_when_the_cookies_cannot_be_saved` — pre ставит глобальную и ждёт `await new
    Promise((r) => setTimeout(r, 300))`, адрес — закрытый порт; пока скрипт ждёт, таблица `app_state`
    переименована вторым соединением `rusqlite` → команда отвечает `Ok`: `error.kind == "connect"`,
    `variable_changes` содержит изменение, глобальная в базе (отправки без изменений переменных ведут себя как
    раньше — их существующие тесты не меняются);
  - `test_results_do_not_bloat_the_history` — pre запроса: двенадцать `pm.test`, каждый бросает `new
    Error('я'.repeat(10000))` (около 240 КБ текста — под пределом фазы, отправка идёт дальше); test запроса:
    `for (let i = 0; i < 1000; i++) pm.test('t' + i, () => { throw new Error('я'.repeat(10000)) })` → ответ
    получен; в `script_errors` одна ошибка с `their results are larger than 256 KB`; длина колонки `detail` этой
    записи истории (читается вторым соединением) не больше 2 × `BODY_LIMIT` истории + 1 МиБ;
  - существующий `warnings_and_network_error` — ожидание предупреждения о скриптах заменить (в отчёт задачи).
  `core_tree.rs`: `created_and_duplicated_collections` — созданная разрешена (`scripts_allowed(id) == true`);
  копия разрешённой разрешена; копия импортированной — нет; `scripts_allowed` и `set_scripts_allowed` для
  неизвестного узла — `Err`.
  `core_session.rs` (в этом файле уже есть ядро над файлом базы — `open(dir)` — и чтение `app_state` вторым
  соединением — `raw_get`; тесты `core_tree.rs` идут над хранилищем в памяти, второго соединения к нему нет):
  `deleted_collection_leaves_the_allowed_list` — две коллекции созданы `create_node`; `raw_get(db,
  "scripts_allowed")` — JSON-массив с двумя id; после `delete_node` первой — массив только со вторым id.
  `core_history.rs`: `entry_keeps_tests_and_script_errors` (в JSON `history_get` — `tests` с пройденным и
  проваленным, у проваленного `error`; `script_errors` со строкой); `entries_written_before_stage_2_are_read`
  (строка, вставленная вторым соединением: `detail` без `tests` и `script_errors`, с `warnings:
  [{"kind":"scripts_not_executed","items":["Scripted"]}]` → `history_list` и `history_get` успешны, `tests ==
  []`, вид предупреждения прежний); `console_is_not_stored` (скрипт печатает `s3cr3t-console`; текст колонки
  `detail` его не содержит и ключа `console` в нём нет); существующую проверку на строке ~230 исправить.
  `core_data.rs`: в `commands_are_listed_everywhere` — `scripts_allowed`, `set_scripts_allowed`, число 39.
  Окно: `RequestEditor.test.ts` — вместо проверки текста надписи: в разделе Scripts элемента `.note` нет;
  `ResponsePanel.test.ts` — предупреждения `scripts_disabled` и `scripts_not_allowed` показаны своими текстами и
  выделены; `History.test.ts` — старая запись с `scripts_not_executed` показывает текст «отправка сделана до
  появления выполнения скриптов»; `sending.test.ts` — результат отмены содержит пустые `tests`, `console`,
  `script_errors`, `variable_changes`.
- [ ] **Step 2: Запустить** `cargo test -p requestmaster` и в `ui/` `npm run test` — новые случаи падают.
- [ ] **Step 3: Реализовать**; поиск мёртвого кода из правил — результаты в отчёт.
- [ ] **Step 4: `cargo test --workspace`, clippy, `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: скрипты исполняются в приложении — результат отправки, история, согласие».

---

### Task 12: Оболочка — настройки скриптов и отчёт импорта

**Files:**
- Modify: `src-tauri/src/dto.rs`, `settings.rs`, `core.rs`, `scripts.rs`, `lib.rs`; `crates/rqm-runner/src/lib.rs`
  (`pub use rqm_script::unsupported_api;`)
- Modify: `ui/src/lib/types.ts`, `testing/fakeApi.ts`, `strings.ts`, `ui/src/components/ImportReport.svelte`,
  `ui/src/components/SettingsDialog.svelte` (только пронос двух значений, без новых элементов управления)
- Test: `src-tauri/tests/core_session.rs`, `core_send.rs`, `core_data.rs`; `ui/src/components/Sidebar.test.ts`,
  `ui/src/components/Settings.test.ts`, `ui/src/lib/motion.test.ts`

**Interfaces:**
- Consumes: `AppSettings { scripts_enabled, script_timeout }` (Task 10); `unsupported_api` (Task 9); записи
  отчёта `HasScripts` из `rqm_postman::import_collection` (у каждой — `node_id`, `node_path`).
- Produces:

```rust
// dto.rs
pub struct SettingsDto { /* timeout_ms, verify_tls, proxy, motion */
                         #[serde(default = "…")] pub scripts_enabled: bool,       // по умолчанию true
                         #[serde(default = "…")] pub script_timeout_ms: u64 }     // по умолчанию 10_000
pub enum ReportKind { /* … */ /// The scripts of the node use sandbox API the engine does not support.
                      UnsupportedScriptApi }
// settings.rs, re-exported from lib.rs
pub const MIN_SCRIPT_TIMEOUT_MS: u64 = 100;
pub const MAX_SCRIPT_TIMEOUT_MS: u64 = 600_000;
pub const DEFAULT_SCRIPT_TIMEOUT_MS: u64 = 10_000;
```

```ts
// types.ts: SettingsDto + scripts_enabled: boolean; script_timeout_ms: number
//           ReportItem.kind + "unsupported_script_api"
```

**Правила:**
- `save_settings`: `script_timeout_ms` вне `MIN…MAX` → `Err("script timeout is out of range")`, сохранённое
  прежнее. `From<SettingsDto> for AppSettings` передаёт оба поля (`script_timeout` —
  `Duration::from_millis`). Значения по умолчанию — и в `default_settings`, и при чтении старого значения без
  этих ключей.
- Импорт коллекции: для каждого узла с записью `HasScripts` — `unsupported_api` по обоим его скриптам, имена
  объединяются без повторов по алфавиту; не пусто → сразу за записью `HasScripts` этого узла добавляется
  `ReportItem { kind: UnsupportedScriptApi, node_id, node_path, detail: имена через ", " }`.
- Окно: `ImportReport.svelte` — вид `unsupported_script_api` в списке `KINDS` после `has_scripts`; подпись группы
  рядом с подписью `has_scripts` в `strings.ts` — «Не поддерживается в скриптах».
- Окно, `SettingsDto`: два новых ключа в TS-типе **обязательные** (тип совпадает с контрактом команд), поэтому в
  этой же задаче правятся все места, где `SettingsDto` собран литералом: `fakeApi.ts` — значение `settings` по
  умолчанию получает `scripts_enabled: true, script_timeout_ms: 10000`; `SettingsDialog.svelte` — в `onMount`
  запоминает прочитанные `settings.scripts_enabled` и `settings.script_timeout_ms` в двух переменных состояния и
  возвращает их без изменений в `api.saveSettings({ … })` (сейчас строка 105 собирает объект из четырёх ключей —
  без проноса «Сохранить» между Task 12 и Task 15 молча сбрасывал бы оба значения). Галочка и поле ввода —
  Task 15. Необязательными (`?`) ключи не делать.
- Rust, полные литералы `SettingsDto { timeout_ms, verify_tls, proxy, motion }` перестают собираться и получают
  два новых поля: `src-tauri/src/settings.rs` (`default_settings`), `src-tauri/tests/core_send.rs` (три литерала
  `save_settings(...)`, сейчас строки 309, 319, 333), `src-tauri/tests/core_session.rs` (`default_settings_value`
  и литералы без `..default_settings_value()` — сейчас строки 100, 305, 348, 361, 380, 430, 458). В тестах —
  значения по умолчанию (`scripts_enabled: true, script_timeout_ms: 10_000`), если тест не про скрипты.
- `core_session.rs`: два литерала `AppSettings` (после Task 10 в них стоит `..AppSettings::default()`) получают
  явные ожидаемые `scripts_enabled` и `script_timeout` — те, что заданы в `SettingsDto` этого теста.

- [ ] **Step 1: Падающие тесты.**
  `core_session.rs`: `default_settings` — JSON `{"timeout_ms":0,"verify_tls":true,"proxy":{"mode":"system"},
  "motion":"normal","scripts_enabled":true,"script_timeout_ms":10000}`; `settings_saved_before_stage_2_keep_
  their_values` — в базе `{"timeout_ms":30000,"verify_tls":false,"proxy":{"mode":"off"},"motion":"rich"}` →
  `get_settings` отдаёт эти четыре значения и `scripts_enabled: true`, `script_timeout_ms: 10000`;
  `script_timeout_out_of_range_is_refused` — `99` и `600001` → `Err("script timeout is out of range")`,
  сохранённое прежнее; `100` и `600000` принимаются; `settings_dto_maps_to_app_settings` — дополнить двумя
  полями: `SettingsDto { scripts_enabled: false, script_timeout_ms: 2500, .. }` → `AppSettings {
  scripts_enabled: false, script_timeout: 2500 мс, .. }` (явно, без `..AppSettings::default()`).
  `core_send.rs`: `scripts_disabled_in_the_settings` — `scripts_enabled: false`, коллекция разрешена →
  `warnings[0] == {"kind":"scripts_disabled","items":[]}`, скрипт не исполнялся; `saved_script_timeout_stops_a_
  script` — `script_timeout_ms: 200`, pre `while (true) {}` → `error.kind == "script"`, `script_errors[0].kind ==
  "timeout"`; `native_loop_is_stopped_by_the_watchdog_and_the_send_is_recorded` — `script_timeout_ms: 200`, pre
  `pm.globals.set('a', '1'); [].reverse.call({ length: 2 ** 53 - 1 })` → команда отвечает не позже чем через
  200 мс + 2 с + 1 с: `error.kind == "script"`, `script_errors[0].kind == "timeout"`; глобальная `a` в базе и в
  `variable_changes`; запись истории этой отправки есть (`error_kind == "script"`); следующая отправка (без
  скриптов) проходит.
  `core_data.rs`: `import_report_names_unsupported_script_api` — синтетическая коллекция: у запроса test-скрипт
  `pm.vault.get('a'); const m = require('moment'); pm.test('x', () => {})`, у папки — `pm.test('y', () => {})` →
  у запроса сразу за `has_scripts` идёт `{"kind":"unsupported_script_api","detail":"pm.vault, require('moment')"}`
  с теми же `node_id` и `node_path`; у папки такой записи нет.
  `Sidebar.test.ts`: отчёт импорта показывает группу «Не поддерживается в скриптах» с текстом `detail`.
  `Settings.test.ts`: два ключа (`scripts_enabled: true, script_timeout_ms: 10000`) дописываются во все литералы
  `SettingsDto` на входе `openSettings(...)` и во все точные ожидания `saveSettings` (`toHaveBeenCalledWith` /
  `toHaveBeenLastCalledWith` сравнивают объект целиком — сейчас это строки 95, 133, 138, 147, 176, 186, 197,
  244, 250, 265, 276); новый случай `script_settings_are_kept_as_read` — настройки с `scripts_enabled: false,
  script_timeout_ms: 2000` после «Сохранить» уходят в `saveSettings` с теми же двумя значениями.
  `motion.test.ts`: литерал на строке 8 получает два ключа.
- [ ] **Step 2: Запустить** `cargo test -p requestmaster` и `npm run test -- src/components/Sidebar.test.ts
  src/components/Settings.test.ts src/lib/motion.test.ts`, затем `npm run check` — новые случаи падают, а
  `check` называет каждый литерал без новых ключей.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `cargo test --workspace`, clippy, `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: настройки скриптов и неподдержанный API в отчёте импорта».

---

### Task 13: Окно — изменения переменных из скриптов

**Files:**
- Create: `ui/src/lib/variableSync.ts`, `ui/src/lib/variableSync.test.ts`
- Modify: `ui/src/lib/sending.svelte.ts`, `ui/src/lib/tabs.svelte.ts`, `ui/src/App.svelte`
- Test: `ui/src/lib/sending.test.ts`, `ui/src/components/Environments.test.ts`, `CollectionEditor.test.ts`,
  `Variables.test.ts`

**Interfaces:**
- Consumes: `SendResult.variable_changes: VariableChange[]` (Task 11); `tabs.environment(id)`,
  `EnvironmentTab.{variables, edited, flush}`, `Tab.{saved, draft, nodeId, kind}`, `workspace.
  activeEnvironmentId`, `bumpVariables()`, `VariableRow`.
- Produces:

```ts
// lib/sending.svelte.ts
export interface SendObserver {
  /** Before the command of a send is called; the send waits for it. A failure does not stop the send. */
  before(api: Api): Promise<void>;
  /** The command returned a result — also one the tab drops (the send was cancelled or replaced). */
  settled(api: Api, result: SendResult): void;
}
/** Registers an observer of every send of the window; the returned function removes it. */
export function observeSends(observer: SendObserver): () => void;
// lib/variableSync.ts
/** The rule of the core (`apply_to_variables`) for the rows of an open editor; changes `rows` in place. */
export function applyToRows(rows: VariableRow[], change: VariableChange): void;
/** Keeps the open editors in step with what scripts change; the returned function stops it. */
export function installVariableSync(): () => void;
// lib/tabs.svelte.ts
class EnvironmentTab { /** Changes a script made to this set: put into the rows and saved like an edit. */
                       applyChanges(api: Api, changes: VariableChange[]): void }
class Tab { /** Changes a script made to the variables of this collection: put into `saved` and into `draft`. */
            applyVariableChanges(changes: VariableChange[]): void }
```

**Поведение:**
- `applyToRows` — то же правило, что в ядре: `set` — у последней строки с `enabled` и этим `key` меняется
  `value`; нет такой — в конец `{ key, value, secret: false, enabled: true, postman_extra: {} }`; `unset` —
  убираются включённые строки с ключом; `clear` — все включённые. Нетронутые строки остаются теми же объектами
  (показанный секрет остаётся показанным).
- `Sending.send`: перед `api.sendRequest` — `await observer.before(api)` каждого наблюдателя; когда команда
  вернула результат — **сначала** `observer.settled(api, result)`, потом прежняя проверка «этот ли результат
  ждали». Команда упала — `settled` не вызывается: ядро гарантирует, что отправка, записавшая изменения
  переменных, отвечает результатом с дельтой, а не ошибкой команды (Task 11, решение № 41), поэтому при отказе
  команды применять нечего. `sending.svelte.ts` не импортирует `tabs` (иначе цикл
  импортов `sending → tabs → sending`).
- `installVariableSync` (вызывается в `App.svelte` при старте, останавливается при размонтировании): `before` —
  `flush` открытой вкладки активного окружения и вкладки Globals (спека §9: скрипт видит то, что на экране);
  `settled` — изменения раскладываются по `target_id`: `scope == "collection"` → открытая вкладка узла с таким
  `nodeId` → `applyVariableChanges`; иначе `tabs.environment(target_id)?.applyChanges(api, …)`; если изменений
  было хоть одно — один раз `bumpVariables()`.
- `EnvironmentTab.applyChanges`: `applyToRows` по `this.variables`, затем `this.edited(api)` — обычная автозапись
  всего набора. Она повторяет то, что уже записало ядро, и чинит случай, когда автозапись вкладки пришла в базу
  между записью ядра и приходом результата.
- `Tab.applyVariableChanges`: `saved` заменяется копией с изменёнными `variables`; `draft.variables` меняется на
  месте. `dirty` от скрипта не меняется; несохранённые правки остальных полей целы; если пользователь правил то
  же значение — в черновике остаётся значение скрипта.

- [ ] **Step 1: Падающие тесты.**
  `variableSync.test.ts`: `rule_matches_the_core` — та же таблица, что `one_rule_for_set_unset_clear` в Task 10
  (те же исходные строки и ожидания); `untouched_rows_keep_their_identity`.
  `sending.test.ts`: `observer_runs_before_and_after_a_send` (порядок: `before`, `sendRequest`, `settled`);
  `settled_is_called_for_a_cancelled_send` (отмена; затем команда отвечает → `settled` вызван с её результатом,
  вкладка показывает «отменён»); `settled_is_called_for_a_replaced_send`; `failing_before_does_not_stop_the_send`.
  `Environments.test.ts`: `script_change_reaches_the_open_environment_tab` — вкладка окружения `a=1`; отправка
  возвращает `variable_changes: [{ scope: "environment", target_id, op: "set", key: "token", value: "T" }]` → в
  таблице строка `token` = `T`; через 400 мс `saveEnvironment` вызван с обеими строками;
  `unsaved_edit_is_flushed_before_the_send` — правка строки `a` и сразу Send → `saveEnvironment` вызван раньше
  `sendRequest`; `edit_made_during_the_send_survives` — отправка ждёт; пользователь меняет `a`; отправка
  возвращает изменение `b` → в таблице изменённая `a` и `b`, запись — с обеими; `shown_secret_stays_shown`.
  `CollectionEditor.test.ts`: `script_change_reaches_a_clean_collection_tab` (переменная в таблице, вкладка не
  помечена несохранённой); `script_change_keeps_unsaved_edits_of_a_dirty_tab` (изменены описание и значение
  другой переменной; приходит `set token` → вкладка по-прежнему несохранённая; Ctrl+S → в `saveNodeContent` ушли
  `token` скрипта, значение пользователя и описание).
  `Variables.test.ts`: `highlight_is_refreshed_after_a_script_changed_variables` — после отправки с изменениями
  `variablesInScope` вызван ещё раз; после отправки без изменений — нет.
- [ ] **Step 2: Запустить** `npm run test -- src/lib/variableSync.test.ts src/lib/sending.test.ts
  src/components/Environments.test.ts src/components/CollectionEditor.test.ts src/components/Variables.test.ts` —
  новые случаи падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: изменения переменных из скриптов доходят до открытых редакторов».

---

### Task 14: Окно — вкладки Tests и Console, ошибки скриптов, тесты в истории

**Files:**
- Create: `ui/src/components/TestResults.svelte`, `ui/src/components/ConsoleView.svelte`
- Modify: `ui/src/components/ResponsePanel.svelte`, `HistoryEntryView.svelte`, `ui/src/lib/strings.ts`,
  `ui/src/styles/base.css` (значок счётчика тестов), при новой паре цветов — `ui/src/styles/contrast.test.ts`
- Test: `ui/src/components/ResponsePanel.test.ts`, `ui/src/components/History.test.ts`

**Interfaces:**
- Consumes: `SendResult { tests, console, console_truncated, script_errors }`, `HistoryEntry { tests,
  script_errors }`, `TestResult`, `ConsoleLine`, `ScriptError`, `ScriptSource` (Task 11); общие стили
  `.section-tabs`, `.count` (`base.css`); `EmptyState`; токены `--status-ok`, `--status-client`, `--danger`,
  `--text-muted`.
- Produces:

```ts
// TestResults.svelte
interface Props { tests: TestResult[]; errors: ScriptError[] }
// ConsoleView.svelte
interface Props { lines: ConsoleLine[]; truncated: boolean }
// strings.ts
// response.tests = "Tests", response.console = "Console"
// tests: { empty, emptyText, passed: "PASS", failed: "FAIL", skipped: "SKIP",
//          summary(passed, failed, skipped), where(phaseLabel, nodeName), scriptError(where),
//          position(line, column), kinds: { exception, timeout, memory, cancelled, internal } }
// console: { empty, emptyText, truncated, at(ms), levels: { log, info, warn, error, debug, network } }
```

**Поведение:**
- `ResponsePanel`: `type Section = "body" | "headers" | "cookies" | "tests" | "console"`. Доступные разделы: есть
  ответ — все пять; ответа нет (ошибка) — `tests`, когда есть тесты или ошибки скриптов, и `console`, когда есть
  строки. Полоса разделов рисуется, когда доступен хотя бы один; при ошибке блок `.failure` остаётся над ней.
  Показывается выбранный раздел, а если он сейчас недоступен — первый доступный; сам выбор при этом не
  перезаписывается (вернётся со следующим ответом). Ctrl+F по-прежнему открывает `body`.
- Три состояния теста: пропущен — `skipped`; пройден — `passed && !skipped`; провален — `!passed`.
- Значки: `headers`, `cookies`, `console` — число, как сейчас; `tests` — текст `пройдено/всего` (пропущенные в
  «всего» не входят), класс `ok`, когда проваленных и ошибок скриптов нет, иначе `bad`; ошибки есть, а тестов
  нет — `!` с `bad`; тестов и ошибок нет — значка нет. Цвет `ok` — `--status-ok`, `bad` — `--danger`. Значок
  проявляется (`opacity`, `--dur-2`).
- Ошибка отправки вида `script` — текст `strings.response.errors.script` и сообщение; вид `skipped` — как
  `cancelled`: не оформляется как ошибка.
- `TestResults`: сверху — ошибки скриптов, каждая блоком: заголовок `strings.tests.scriptError(where)`
  («Ошибка в скрипте Pre-request · Login»), для видов кроме `exception` — `strings.tests.kinds[kind]`
  («превышен предел времени», «превышен предел памяти», «отменён», «сбой движка»), при `line != null` —
  `strings.tests.position(line, column)` («строка 3, столбец 5»), текст сообщения в `<pre>`. Затем итог
  `strings.tests.summary` («Пройдено 2, провалено 1, пропущено 0») и строки тестов: слово статуса цветом, имя,
  у скрипта не уровня запроса — приглушённое `where`, у проваленного — текст ошибки. Пусто — `EmptyState`.
  `where` — `strings.tests.where(strings.scripts.kinds[phase], node_name)`.
- `ConsoleView`: строка — время `strings.console.at(at_ms)` («+12 мс»), уровень `strings.console.levels[level]`,
  текст моноширинным с `white-space: pre-wrap` и переносом длинных слов, у строки скрипта — приглушённое
  `where`. Цвета: `warn` — `--status-client`, `error` — `--danger`, `debug` и `network` — `--text-muted`.
  `truncated` → пометка `strings.console.truncated` («Вывод обрезан: показаны первые 1000 строк или 1 МБ»).
  Пусто — `EmptyState`.
- Весь текст из скриптов выводится как `{text}`.
- `HistoryEntryView`: под сводкой и предупреждениями — `TestResults`, когда в записи есть тесты или ошибки
  скриптов. Консоли в записи нет.

- [ ] **Step 1: Падающие тесты.**
  `ResponsePanel.test.ts`:
  - `tests_and_console_tabs_with_badges` — результат с ответом, тремя тестами (два пройдены, один провален с
    `AssertionError: expected 500 to equal 200`) и двумя строками консоли → вкладка `Tests` со значком `2/3`
    класса `bad`, `Console` со значком `2`; в `Tests` — `PASS`, `FAIL` и текст ошибки; в `Console` — уровень и
    текст каждой строки;
  - `all_passed_badge_is_ok`; `response_without_tests_has_no_badge_and_an_empty_state`;
  - `script_error_is_shown_with_its_place` — `error: { kind: "script", message: "TypeError: x is not a function"
    }`, `script_errors: [{ source: { node_name: "Col", node_kind: "collection", phase: "pre_request", .. }, kind:
    "exception", message: "TypeError: x is not a function", line: 3, column: 5 }]`, ответа нет → текст «Ошибка в
    pre-request скрипте — запрос не отправлен»; в полосе только `Tests` (значок `!`); в нём «Pre-request · Col»,
    «строка 3, столбец 5» и сообщение;
  - `timeout_error_names_its_kind` — `kind: "timeout"` → «превышен предел времени»;
  - `chosen_section_returns_with_the_next_response` — выбрать `Tests`; следующий результат — сетевая ошибка без
    тестов (полосы нет); следующий — ответ с тестами → показан `Tests`;
  - `script_output_is_plain_text` — строка консоли `<img src=x onerror=alert(1)>` и имя теста `<b>x</b>` →
    элементов `img` и `b` в панели нет, текст виден как набран;
  - `truncated_console_note`; `skipped_request_is_not_an_error`.
  `History.test.ts`: `entry_shows_its_tests_and_script_errors`; `entry_without_tests_shows_no_tests_block`.
- [ ] **Step 2: Запустить** `npm run test -- src/components/ResponsePanel.test.ts src/components/History.test.ts`
  — новые случаи падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные (в том числе `csp.test.ts` и
  `contrast.test.ts`).
- [ ] **Step 5: Commit** — «Этап 2: вкладки Tests и Console, ошибки скриптов со строкой, тесты в истории».

---

### Task 15: Окно — согласие на скрипты и настройки скриптов

**Files:**
- Modify: `ui/src/components/ImportReport.svelte`, `CollectionEditor.svelte`, `SendWarnings.svelte`,
  `ResponsePanel.svelte`, `SettingsDialog.svelte`, `ui/src/App.svelte`, `ui/src/lib/strings.ts`
- Test: `ui/src/components/Sidebar.test.ts`, `CollectionEditor.test.ts`, `ResponsePanel.test.ts`,
  `History.test.ts`, `Settings.test.ts`

**Interfaces:**
- Consumes: `api.scriptsAllowed`, `api.setScriptsAllowed`, `FakeSeed.scriptsAllowed`, `FakeApi.allowCalls`
  (Task 11); `SettingsDto { scripts_enabled, script_timeout_ms }` и их пронос в `SettingsDialog` — диалог уже
  читает оба значения и возвращает их в `saveSettings` (Task 12), здесь добавляются только элементы управления
  и проверка диапазона; `ImportResult { kind, root_id,
  report }`; предупреждение `scripts_not_allowed`.
- Produces:

```ts
// SendWarnings.svelte
interface Props { warnings: Warning[];
                  /** When given, the warning `scripts_not_allowed` gets the button «Разрешить». */
                  onallow?: () => void }
// ResponsePanel.svelte
interface Props { sending: Sending; findSignal?: number;
                  /** The request whose response is shown: its collection gets the consent. Without it the
                   *  warning has no button. */
                  nodeId?: string }
```

**Поведение:**
- `ImportReport`: у каждого результата вида `collection`, в отчёте которого есть запись `has_scripts`, над
  записями — блок: заголовок «В коллекции есть скрипты»; текст «Скрипты выполняются при каждой отправке запроса.
  Они читают все переменные, включая секретные, и могут отправлять запросы на любые адреса. Разрешайте скрипты
  только коллекциям, которым доверяете.»; кнопка «Разрешить скрипты» → `api.setScriptsAllowed(result.root_id,
  true)`; после успеха вместо кнопки — «Скрипты разрешены»; отказ команды — уведомление, кнопка остаётся.
  Закрытие отчёта без нажатия ничего не разрешает.
- `CollectionEditor`, раздел Scripts: над редактором галочка «Разрешить скрипты этой коллекции» с тем же
  предупреждением одной строкой. Значение читается `api.scriptsAllowed(tab.nodeId)` при первом показе раздела;
  щелчок сразу вызывает `api.setScriptsAllowed(tab.nodeId, checked)`. В черновик не входит: вкладка от этого не
  становится несохранённой. Отказ команды — галочка возвращается, уведомление.
- `ResponsePanel` передаёт в `SendWarnings` `onallow`: `api.setScriptsAllowed(nodeId, true)`, затем уведомление
  «Скрипты разрешены — отправьте запрос ещё раз». `App.svelte` передаёт `nodeId` вкладки. В `HistoryEntryView`
  `onallow` не передаётся — кнопки нет.
- `SettingsDialog`: галочка «Выполнять скрипты»; поле «Лимит времени скрипта, мс» с пояснением «От 100 до
  600 000» и текстом ошибки «Целое число от 100 до 600 000»; при неверном значении «Сохранить» ничего не пишет.
- Все тексты — в `strings.ts` (`import.scriptsTitle`, `import.scriptsText`, `import.allowScripts`,
  `import.scriptsAllowed`, `collection.allowScripts`, `collection.allowScriptsNote`, `response.allowScripts`,
  `response.scriptsAllowedNow`, `settings.scriptsEnabled`, `settings.scriptTimeout`, `settings.scriptTimeoutNote`,
  `settings.scriptTimeoutInvalid`).

- [ ] **Step 1: Падающие тесты.**
  `Sidebar.test.ts`: `collection_with_scripts_asks_for_consent` — импорт двух коллекций, со скриптами и без →
  блок и кнопка только у первой; щелчок → `allowCalls == [[root_id, true]]`, вместо кнопки «Скрипты разрешены»;
  `closing_the_report_allows_nothing`.
  `CollectionEditor.test.ts`: `allow_scripts_acts_at_once_and_is_not_part_of_the_draft` — галочка отражает
  `FakeSeed.scriptsAllowed`; щелчок → `allowCalls` пополнился, вкладка не помечена несохранённой, `saveDraft` из-за
  этого не вызван.
  `ResponsePanel.test.ts`: `not_allowed_warning_has_an_allow_button` — щелчок → `allowCalls == [[nodeId, true]]`
  и уведомление; у предупреждения `scripts_disabled` кнопки нет.
  `History.test.ts`: `history_entry_has_no_allow_button`.
  `Settings.test.ts`: `scripts_settings_are_saved` — снять галочку, ввести `2000`, «Сохранить» → `saveSettings`
  получил `scripts_enabled: false`, `script_timeout_ms: 2000`; `script_timeout_out_of_range_is_not_saved` — `99`
  → текст ошибки, `saveSettings` не вызван.
- [ ] **Step 2: Запустить** `npm run test -- src/components/Sidebar.test.ts src/components/CollectionEditor.test.ts
  src/components/ResponsePanel.test.ts src/components/History.test.ts src/components/Settings.test.ts` — новые
  случаи падают.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: согласие на скрипты коллекции и настройки скриптов в окне».

---

### Task 16: Окно — автодополнение `pm.*` в редакторе скриптов

**Files:**
- Create: `ui/src/lib/scriptApi.ts`, `ui/src/lib/codemirror/scriptCompletions.ts`,
  `ui/src/lib/codemirror/scriptCompletions.test.ts`
- Modify: `ui/src/components/CodeEditor.svelte`, `ui/src/lib/testing/CodeEditorStub.svelte`,
  `ui/src/components/ScriptsEditor.svelte`, `ui/src/lib/strings.ts`, `ui/vitest.config.ts` (`server.fs.allow`)
- Test: `ui/src/components/RequestEditor.test.ts`

**Interfaces:**
- Consumes: `crates/rqm-script/prelude/src/api.json` (Task 9) — только в тесте; `CompletionContext`,
  `CompletionResult` из `@codemirror/autocomplete`; образец источника и его подключения —
  `ui/src/lib/codemirror/variables.ts` (`EditorState.languageData.of(() => [{ autocomplete: … }])`).
- Produces:

```ts
// lib/scriptApi.ts
export interface ScriptApiItem { name: string; kind: "function" | "property"; hint?: string }
/** Every name the script engine supports: the `v1` and `noop` members of the table of the core. */
export const SCRIPT_API: readonly ScriptApiItem[];
// lib/codemirror/scriptCompletions.ts
/** Completion source of a script editor: the members of the sandbox API after a dot, its top-level names
 *  while one is typed. `null` — nothing to offer. */
export function scriptCompletions(context: CompletionContext): CompletionResult | null;
// CodeEditor.svelte, CodeEditorStub.svelte
completions?: "script"        // CodeEditorStub: data-completions={completions ?? "none"}
```

**Поведение:**
- Текст до каретки сопоставляется с `/[\w$]+(?:\.[\w$]*)*$/`; не совпал — `null`. Последняя точка делит его на
  путь и начало имени. Варианты — различные следующие сегменты имён `SCRIPT_API`, которые начинаются с `путь +
  "."` (при пустом пути — первые сегменты всех имён) и сами начинаются с набранного начала (с учётом регистра);
  `from` — позиция начала имени. Без точки список даётся, только когда начало имени не пусто или запрос явный
  (`context.explicit`, Ctrl+Пробел). Вариантов нет — `null`.
- Вариант-лист: `type` — `function` или `property`, `detail` — `hint`. Сегмент, у которого есть продолжение, —
  `type: "namespace"`.
- `CodeEditor` с `completions="script"` подключает источник своим `Compartment`; свойство передаёт только
  `ScriptsEditor`. Тела запросов, ответ и история подсказок `pm.*` не получают.
- `strings.shortcuts.editorVariables` — «Список подсказок — в редакторе и в полях с переменными».

- [ ] **Step 1: Падающие тесты.**
  `scriptCompletions.test.ts`: `list_matches_the_table_of_the_core` — файл таблицы читается тем же приёмом, что
  в остальных тестах окна (`csp.test.ts`, `contrast.test.ts`): `import apiTable from
  "../../../../crates/rqm-script/prelude/src/api.json?raw"` и `JSON.parse(apiTable)`. `node:fs` не использовать:
  в `ui/` нет `@types/node`, а `tsconfig.json` проверяет и тесты — `npm run check` упал бы; новых зависимостей в
  окне нет. Файл лежит вне корня Vite (`ui/`), и без настройки vitest отвечает на такой импорт `Denied ID …`
  (проверено опытом вне репозитория на vitest этого проекта), поэтому в `ui/vitest.config.ts` добавляется
  `server: { fs: { allow: [".", "../crates/rqm-script/prelude/src"] } }` с комментарием, зачем (с ней тот же
  импорт проходит — проверено там же); `vite.config.ts` продукта не трогается. Множество имён элементов `members` со статусом `v1` или `noop` равно множеству имён
  `SCRIPT_API`, виды совпадают; `members_after_a_dot` — на состоянии с текстом `pm.` и кареткой в конце: среди
  вариантов `environment`, `test`, `response`, `sendRequest`, `from == 3`; `pm.environment.s` → ровно `set` с
  `detail: "set(key, value)"`; `pm.response.to.have.` → есть `status`; `postman.` → есть
  `setEnvironmentVariable`; `top_level_names` — `p` → есть `pm` и `postman`; пустой текст без явного запроса →
  `null`, с явным — есть `pm`, `console`, `tests`, `responseBody`; `foo.` → `null`; `x = 1` → `null`;
  `unsupported_names_are_not_offered` — `pm.va` → среди вариантов нет `vault`.
  `RequestEditor.test.ts`: у редактора скрипта `data-completions="script"`, у редактора тела — `"none"`.
- [ ] **Step 2: Запустить** `npm run test -- src/lib/codemirror/scriptCompletions.test.ts
  src/components/RequestEditor.test.ts` — новые случаи падают. `list_matches_the_table_of_the_core` должен падать
  из-за расхождения списков (или отсутствия `scriptApi.ts`), а **не** из-за ошибки загрузки `api.json` — если
  в выводе `Denied ID` или ошибка разбора импорта, сначала чинится `server.fs.allow`. Если импорт вне корня
  не удаётся и с этой настройкой — остановиться и спросить владельца (варианты: `@types/node` как
  dev-зависимость — отступление от «новых зависимостей нет»; или копия списка имён, сверяемая Rust-тестом в
  `crates/rqm-script/tests/api.rs`).
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: `npm run check`, `npm run test`, `npm run build`** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: автодополнение pm.* в редакторе скриптов».

---

### Task 17: Набор совместимости и образцы владельца

**Files:**
- Create: `crates/rqm-runner/tests/script_compat.rs`
- Modify: `crates/rqm-runner/tests/owner_end_to_end.rs`

**Interfaces:**
- Consumes: весь конвейер (`load_send_input`, `send`, `apply_variable_changes`, `set_scripts_allowed`),
  `rqm_script::{run, Job, Phase, unsupported_api}`, `rqm_postman::import_collection` (dev-зависимость уже есть).
- Produces: только тесты.

**Набор совместимости.** Один помощник: случай = скрипты по уровням (коллекция, папка, запрос; pre и test),
переменные областей, ответ сервера, ожидаемое. Каждый случай идёт через настоящий `send` с wiremock и общим
движком; там, где случай из двух отправок, между ними вызывается `apply_variable_changes` и вход читается заново.
Строки консоли сравниваются без сетевых (`level != Network`).

| № | Имя случая | Скрипт (суть) | Ожидается |
|---|---|---|---|
| 1 | `owner_token_from_body` | test: `var jsonData = JSON.parse(responseBody);` `pm.collectionVariables.set("token", jsonData.token);`; тело `{"token":"abc"}` | `changes == [Collection Set token abc]`, ошибок нет |
| 2 | `status_test` | test: `pm.test("Status code is 200", function () { pm.response.to.have.status(200); });` | ответ 200 → пройден; ответ 500 → провален, `AssertionError: expected response to have status code 200 but got 500` |
| 3 | `legacy_tests_and_headers` | test: `tests["Status code is 200"] = responseCode.code === 200;` `tests["Is json"] = postman.getResponseHeader("Content-Type").indexOf("json") !== -1;` | оба пройдены |
| 4 | `json_value_checks` | test: `const body = pm.response.json(); pm.test("has items", () => { pm.expect(body.items).to.be.an("array").that.has.lengthOf(2); pm.expect(body.items[0]).to.have.property("id", 1); });` | пройден |
| 5 | `response_time_and_header` | test: `pm.test("fast", () => pm.expect(pm.response.responseTime).to.be.below(5000)); pm.test("json", () => pm.response.to.have.header("Content-Type"));` | оба пройдены |
| 6 | `value_from_one_request_feeds_the_next` | отправка 1, test: `pm.environment.set("id", pm.response.json().id)`; отправка 2: адрес `{{host}}/items/{{id}}` | сервер получил `/items/7` |
| 7 | `number_is_text_in_the_next_send` | отправка 1, test: `pm.environment.set("count", 1)`; отправка 2, pre: `console.log(typeof pm.environment.get("count"), pm.environment.get("count") + 1)` | консоль `string 11` (отличие от Postman, решение № 7) |
| 8 | `timestamp_and_nonce_in_pre_request` | pre: `pm.request.headers.upsert({ key: "X-Time", value: String(Date.now()) }); pm.variables.set("nonce", pm.variables.replaceIn("{{$guid}}"));` заголовок `X-Nonce: {{nonce}}` | сервер получил `X-Time` из цифр и `X-Nonce` вида UUID |
| 9 | `collection_script_fetches_a_token_once` | pre коллекции: `if (!pm.environment.get("token")) { const r = await pm.sendRequest(pm.variables.get("host") + "/auth"); pm.environment.set("token", r.json().token); }`; заголовок `Authorization: Bearer {{token}}` | две отправки подряд: `/auth` вызван один раз, обе пришли с токеном |
| 10 | `callback_request_with_a_test_inside` | test: `pm.sendRequest(pm.variables.get("host") + "/check", function (err, res) { pm.test("check ok", function () { pm.expect(err).to.equal(null); pm.expect(res).to.have.status(200); }); });` | тест пройден |
| 11 | `folder_scripts_between_collection_and_request` | у всех уровней `console.log` | порядок из Task 4 |
| 12 | `local_variable_does_not_outlive_the_send` | отправка 1, pre: `pm.variables.set("tmp", "1")`; отправка 2, pre: `console.log(pm.variables.get("tmp"))` | `changes` пуст; консоль `undefined` |
| 13 | `unset_and_clear` | test: `pm.environment.unset("a"); pm.globals.clear();` | в базе нет `a`; включённых глобальных нет, выключенная на месте |
| 14 | `console_objects` | test: `console.log("user", { id: 1 }, [1, 2])` | `user {"id":1} [1,2]` |
| 15 | `error_in_a_test_script_has_its_line` | test: `const a = pm.response.json();` / `a.missing.deep;` | ответ получен; ошибка `Exception`, фаза test, строка 2, начинается с `TypeError` |
| 16 | `syntax_error_in_pre_request` | pre: `pm.variables.set("a", "1");` / `if (` | `Err(SendError::Script)`, строка 2, запрос не ушёл |
| 17 | `await_and_timer` | test: `await new Promise((r) => setTimeout(r, 20)); pm.test("after wait", () => {});` | пройден |
| 18 | `set_next_request_is_ignored` | test: `postman.setNextRequest("Other"); pm.execution.setNextRequest(null); pm.test("ran", () => {});` | пройден, ошибок нет |
| 19 | `skip_request_by_condition` | pre: `if (!pm.environment.get("enabled")) pm.execution.skipRequest();` | `Err(SendError::Skipped)`, запросов нет |
| 20 | `lodash_and_btoa` | test: `const _ = require("lodash"); pm.environment.set("names", _.map(pm.response.json().items, "name").join(",")); pm.environment.set("b64", btoa("user:pass"));` | значения `a,b` и `dXNlcjpwYXNz` |
| 21 | `unsupported_module_is_a_clear_error` | test: `const moment = require("moment");` | ответ получен; ошибка `Error: Cannot find module 'moment' (not supported in RequestMaster)` |
| 22 | `urlencoded_body_built_in_pre_request` | тело urlencoded `a=1`; pre: `pm.request.body.urlencoded.add({ key: "sig", value: "x" })` | сервер получил `a=1&sig=x` |
| 23 | `secret_variable_is_readable` | секретная `password` окружения; pre: `pm.request.headers.add({ key: "X-P", value: pm.environment.get("password") })` | сервер получил значение |
| 24 | `non_json_body_fails_only_the_test` | тело `<html></html>`; test: `pm.test("json", () => pm.response.json()); tests["has html"] = responseBody.indexOf("<html>") === 0;` | `json` провален, начинается с `JSONError`; `has html` пройден |
| 25 | `disabled_variable_is_invisible` | выключенная `off=1` в окружении; test: `console.log(pm.environment.has("off"), JSON.stringify(pm.environment.toObject()))`; `pm.environment.set("off", "2")` | консоль `false {}`; в базе выключенная `off=1` и новая включённая `off=2` |
| 26 | `endless_script_does_not_hang_the_send` | `script_timeout` 300 мс; test: `while (true) {}` | ответ получен; ошибка `Timeout`; следующая отправка работает |
| 27 | `test_reads_the_body_that_was_sent` | тело raw `{"name":"{{userName}}","age":{{age}}}` (`userName` = `bob`, `age` = `5`), сервер возвращает тело запроса; test: `const sent = JSON.parse(pm.request.body.raw); pm.test("echo", () => pm.expect(pm.response.json().name).to.eql(sent.name));` | пройден (в test-скрипте — отправленный запрос, решение № 40) |
| 28 | `send_request_keeps_braces` | переменная `name` = `X`; pre: `await pm.sendRequest({ url: pm.variables.get("host") + "/render", method: "POST", header: { "Content-Type": "application/json" }, body: { mode: "raw", raw: JSON.stringify({ template: "Hello {{name}}" }) } });` | сервер получил на `/render` тело `{"template":"Hello {{name}}"}` дословно (решение № 28) |
| 29 | `legacy_set_is_seen_in_the_environment_object` | test: `var data = JSON.parse(responseBody); postman.setEnvironmentVariable("token", data.token); tests["token saved"] = environment.token === data.token;` | тест пройден; `changes == [Environment Set token abc]` |
| 30 | `assignment_without_var_to_a_legacy_name` | test: `data = JSON.parse(responseBody); tests["has token"] = data.token === "abc";` | тест пройден, ошибок нет |

**Образцы владельца** (`owner_end_to_end.rs`, новый тест `owner_scripts_compile_in_the_engine`): для каждой
коллекции из `spike/samples` (каталога или файлов нет — напечатать `owner samples: 0 (skipped)` и выйти):
импорт; каждый непустой скрипт каждого узла проверяется вызовом `rqm_script::run(Job::new(Phase::Test,
vec![format!("return;\n{code}")]), &CancellationToken::new())` — обёртка компилирует весь текст, а `return`
первой строкой не даёт ничему исполниться; скрипт «собрался», когда `errors` пуст. Печатается **только**
`owner scripts: blocks N, compiled N, failed M, with unsupported api K` (`K` — число скриптов, для которых
`unsupported_api` не пуст). Утверждение: `failed == 0`; в сообщении провала — только номер образца и порядковый
номер скрипта, без имён, текста и значений.

Второй тест там же, `owner_collection_export_survives_a_script_change`: для каждой коллекции образцов — импорт в
хранилище в памяти, экспорт «до»; `apply_variable_changes(.., [Collection Set <ключ первой включённой переменной
коллекции> "rqm-test"])` (у коллекции без переменных — `Set "rqm_probe" "rqm-test"`); экспорт «после». Считается
число путей JSON, по которым «до» и «после» различаются **вне** массива `variable`, и число изменившихся
элементов внутри `variable`. Печатается **только** `owner export after a script change: collections N,
differences outside variable M, changed variable rows K`. Утверждение: `M == 0` и `K == N` (ровно одна строка на
коллекцию: изменённая или добавленная); в сообщении провала — только номер образца и числа.

- [ ] **Step 1: Написать `script_compat.rs`** — 30 случаев таблицы, каждый отдельным `#[tokio::test]` с именем из
  колонки 2, и два теста образцов владельца в `owner_end_to_end.rs`.
- [ ] **Step 2: Запустить** `cargo test -p rqm-runner --test script_compat --test owner_end_to_end -- --nocapture`.
  Случай, который падает, — дефект реализации задач 1–10: исправить в том crate, где он живёт, добавив там же
  тест на причину; ожидание в таблице не менять (если ожидание кажется неверным — остановиться и спросить).
- [ ] **Step 3: В отчёт** — строки `owner scripts: …` и `owner export after a script change: …` (только числа) и
  список исправленных дефектов.
- [ ] **Step 4: `cargo test --workspace` и clippy** — зелёные.
- [ ] **Step 5: Commit** — «Этап 2: набор совместимости скриптов и проверка скриптов образцов».

---

### Task 18: Замеры движка и отправки со скриптами

**Files:**
- Modify: `crates/rqm-script/Cargo.toml` (`[features] measure = []`), `src/lib.rs`, `src/engine.rs`
- Create: `crates/rqm-script/tests/perf.rs`, `crates/rqm-script/tests/perf_cold.rs`,
  `src-tauri/tests/core_perf_cold.rs`, `src-tauri/tests/common/perf.rs`
- Modify: `src-tauri/tests/core_perf.rs`

**Interfaces:**
- Consumes: `run`, `Job`, `Report` (Task 1–9); `Runtime::memory_usage().memory_used_size`; помощники
  `core_perf.rs` (`core_in`, `overhead_ms`, `SENDS = 30`).
- Produces:

```rust
// crates/rqm-script — только с cargo-feature `measure` (по умолчанию выключена, ни один crate продукта её не включает)
// Report: + #[cfg(feature = "measure")] pub memory_used_bytes: u64   // наибольший memory_used_size рантайма скрипта в его конце
```

**Правила:**
- Все тесты-замеры — `#[ignore]`, печатают числа и ничего не утверждают о времени; запускаются вручную и только в
  release (как замеры этапа 1).
- `tests/perf.rs` (`#![cfg(feature = "measure")]`), тест `engine_report`, по 200 вызовов `run` на сценарий, печать
  `engine_ms case=<имя> median=… max=… n=200 memory_kb=…`:
  `engine_start` — скрипт `0` (поток, рантайм, контекст, прелюдия); `owner_like` — фаза test, тело-JSON 15 КБ,
  `var d = JSON.parse(responseBody); pm.collectionVariables.set("token", d.token);`; `typical` — три `pm.test` с
  `pm.expect`, `pm.response.json()` дважды, `console.log`; `chain_of_six` — шесть скриптов `pm.variables.set("a",
  "1")`.
- `tests/perf_cold.rs` — один тест `first_use_in_the_process`: первый в процессе `run` со скриптом `0`, затем
  первый с `pm.expect(1).to.equal(1)`; печать `engine_first_prelude_ms=…` и `engine_first_chai_ms=…`.
- `core_perf.rs`, тест `script_overhead_report`: сценарии `no_scripts`, `blank_scripts` (скрипты из пробелов у
  коллекции и запроса), `owner_like`, `typical` (pre-request коллекции с `pm.variables.set` и test запроса с
  тремя `pm.test` и `pm.expect`); коллекция создаётся `create_node` (разрешена). На **каждый сценарий — новый
  `TempDir` и новый `AppCore`** (`core_in`); сервер wiremock один на тест. На сценарий одна прогревочная отправка
  и 30 измеряемых; печать `script_overhead_ms case=<имя> first=… median=… max=… n=30`. `first` — накладные
  расходы прогревочной отправки, в выборку из 30 она не входит (так же в существующем `request_overhead_report`).
  Байткод прелюдии и chai компилируется один раз на процесс (`OnceLock`), а сценарии идут подряд в одном
  процессе, поэтому у сценариев после первого скриптового `first` — **справочный** и с порогом холодного старта
  не сравнивается. В число входит запись переменных и истории.
- `core_perf_cold.rs` — один `#[ignore]`-тест `first_scripted_send_in_the_process` и больше ничего (один файл —
  один тестовый бинарь — один процесс): первая и единственная отправка в процессе — сценарий `typical`
  (прелюдия и chai обе холодные); печать `script_first_send_ms=…`. Помощники `core_in` и `overhead_ms`
  выносятся в общий модуль тестов `src-tauri/tests/common/perf.rs` (подключается из обоих файлов так же, как уже
  подключён `common/big_collection.rs`: `#[path = "common/perf.rs"] mod perf;`; числа существующих замеров
  `core_perf.rs` от переноса не меняются).

- [ ] **Step 1: Написать тесты-замеры**; обычный `cargo test --workspace` их пропускает и остаётся зелёным;
  `cargo test -p rqm-script --features measure` собирается.
- [ ] **Step 2: `cargo test --workspace` и clippy** (в том числе `cargo clippy -p rqm-script --features measure
  --all-targets -- -D warnings`) — зелёные.
- [ ] **Step 3: Проверить, что программа не запущена** (`tasklist` не показывает `requestmaster.exe` — cargo
  пересобирает и exe пакета). Запущена — Step 4 пропустить, отметить в отчёте (он повторяется в Task 19).
- [ ] **Step 4: Снять числа в release:** `cargo test --release -p rqm-script --features measure --test perf --test
  perf_cold -- --ignored --nocapture --test-threads=1` и `cargo test --release -p requestmaster --test core_perf
  --test core_perf_cold -- --ignored --nocapture --test-threads=1`. Первая release-сборка долгая (`lto`) — не
  зависание. Числа — в отчёт рядом с целями решения № 31:
  - `no_scripts` и `blank_scripts` — медиана не более чем на 1 мс выше медианы `request_overhead_ms`, снятой **в
    этом же запуске** тем же файлом (на этапе 1 она была 5,06 мс — для справки, не для сравнения);
  - `typical` — медиана ≤ 20 мс;
  - холодный старт — `script_first_send_ms` ≤ 50 мс (не `first` сценария `typical`: там прелюдия уже
    скомпилирована предыдущим сценарием). Рядом записать `first_request_overhead_ms` (холодная отправка без
    скриптов, из `request_overhead_report`) и `engine_first_prelude_ms` + `engine_first_chai_ms` — видна доля
    скриптов;
  - `engine_start` — ориентир решения № 4: медиана порядка 0,1 мс; кратно больше (например > 1 мс) — записать
    в отчёт как расхождение с обоснованием решения № 4;
  - `memory_kb` — порога нет, только запись.
  Цель не достигнута — в этой задаче не чинить: записать в отчёт с разбивкой по `engine_ms`.
- [ ] **Step 5: Commit** — «Этап 2: замеры движка скриптов и отправки со скриптами».

---

### Task 19: Сборка, замеры, список проверки для владельца, документы

**Files:**
- Create: `stage2-scripts-results.md`
- Create: `stage2-scripts-owner-checklist.md`
- Create: `THIRD-PARTY-LICENSES.txt`
- Modify: `docs/design.md`, `src-tauri/tauri.conf.json`
  (`bundle.resources`)
- Delete: `spike/script-probe/` (решение № 33; если владелец его отклонил — не удалять)

- [ ] **Step 1: Полный прогон:** `cargo test --workspace`, clippy; в `ui/` — `npm run check`, `npm run test`, `npm
  run build`; `npm audit` в `ui/` и в `crates/rqm-script/prelude/` (ожидаемое замечание о `lodash` `_.template`
  записать: граница безопасности — песочница, а не библиотека). `cargo tree -p <crate> -i tauri` для каждого
  `rqm-*` — пакета нет; в дереве нет aws-lc / native-tls / openssl; `git ls-files spike/samples` — пусто. В
  `crates/rqm-script/prelude/`: `npm run build`, затем `git diff --exit-code -- crates/rqm-script/prelude/built` —
  собранные файлы воспроизводятся. Поиск мёртвого кода: `scripts.note`, `появится на следующем этапе`, `nothing
  runs them yet` — пусто; `ScriptsNotExecuted` и `scripts_not_executed` — только чтение старой истории (`dto.rs`,
  `types.ts`, `strings.ts`, тесты); `__host` — только в `crates/rqm-script`.
- [ ] **Step 2: Замеры без окна** (release; до Step 4): программа не запущена (`tasklist`; запущена — вернуть
  `BLOCKED` с просьбой закрыть). Команды Task 18 Step 4. Записать: `request_overhead_ms` и
  `first_request_overhead_ms` (как на этапе 1), `script_overhead_ms` по четырём сценариям,
  `script_first_send_ms` (холодный старт — отдельный процесс), `engine_ms` по четырём сценариям с памятью,
  `engine_first_*`; рядом — пороги из Task 18 Step 4. В итогах — «снято в release, без окна».
- [ ] **Step 3: Лицензии и пробник.** `THIRD-PARTY-LICENSES.txt`: тексты лицензий QuickJS-NG (MIT), `rquickjs`
  (MIT), `allocator-api2` (MIT OR Apache-2.0 — входит в сборку через `hashbrown`; `relative-path` в сборку не
  входит и в файл не вносится), `chai` 4.4.1 и его семи зависимостей из сборки (`assertion-error`,
  `check-error`, `deep-eql`, `get-func-name`, `loupe`, `pathval`, `type-detect` — MIT; взять из их каталогов в
  `node_modules`), `lodash` 4.18.1 (MIT), и пометка, что тексты сообщений утверждений ответа повторяют `chai-postman` 2.0.1 (Apache-2.0) с
  текстом этой лицензии. В `tauri.conf.json` — `"resources": ["../THIRD-PARTY-LICENSES.txt"]` в разделе `bundle`.
  Пробник: поиск `script-probe` по репозиторию (в том числе `spike/README.md`, спека, комментарии `Cargo.toml`) —
  упоминания исправить, каталог удалить.
- [ ] **Step 4: Собрать приложение:** программа не запущена (иначе `BLOCKED`); в `ui/` — `npm run tauri build`.
  Приложение не запускать. Записать размеры exe и установщика (цель — установщик ≤ 20 МБ; этап 1: exe 9,26 МиБ,
  установщик 3,38 МиБ) и прибавку от движка; проверить, что `THIRD-PARTY-LICENSES.txt` попал в установщик (по
  журналу сборки).
- [ ] **Step 5: Список проверки для владельца** — на русском, простыми словами, пронумерованные шаги с «что
  должно получиться», по разделам:
  1. *Обновление:* данные и история на месте; таймаут, прокси и SSL в настройках прежние; старая запись истории
     со скриптами открывается и говорит «отправка сделана до появления выполнения скриптов».
  2. *Свои скрипты (главный сценарий):* запрос коллекции, чей test-скрипт кладёт значение из ответа в переменную
     коллекции → Send → полоса «Скрипты не выполнены: для коллекции … не разрешены» → «Разрешить» → Send →
     значение переменной изменилось (вкладка коллекции, подсветка `{{…}}` в другом запросе), следующий запрос
     уходит с новым значением.
  3. *Открытые редакторы:* вкладка коллекции открыта, в описании несохранённая правка → Send → переменная
     обновилась, точка «не сохранено» осталась → Ctrl+S → значение скрипта на месте. Вкладка окружения открыта →
     скрипт с `pm.environment.set` → строка появилась или обновилась; показанный секрет остался показанным;
     правка, сделанная во время долгой отправки, не пропала. Несохранённый скрипт во вкладке **коллекции** не
     исполняется, пока не нажат Ctrl+S; несохранённый скрипт запроса — исполняется.
  4. *Tests:* `pm.test` с прошедшей и упавшей проверкой — значок `1/2` красный, текст ошибки; все прошли — значок
     зелёный; ошибка в скрипте на первой, средней и последней строке — номер строки совпадает с номером в
     редакторе; синтаксическая ошибка в pre-request — запрос не ушёл, ошибка показана.
  5. *Console:* `console.log` строки, числа, объекта; уровни `warn` и `error` цветом; время; строка сети основного
     запроса; pre-request коллекции с `pm.sendRequest` за токеном — строка сети запроса скрипта и токен в
     заголовке основного запроса.
  6. *Пределы:* скрипт `while (true) {}` → через 10 с ошибка «превышен предел времени», программа отвечает;
     Cancel во время такого скрипта — сразу; лимит 1000 мс в настройках — ошибка через секунду; `while (true)
     console.log(1)` → «Вывод обрезан»; `while (true) pm.globals.set('k', 'x'.repeat(1 << 20))` → ошибка
     «превышен предел памяти», программа жива, в Globals значение не длиннее 1 МБ. Скрипт
     `[].reverse.call({ length: 2 ** 53 - 1 })` (движок не может его прервать) при лимите 1000 мс → ошибка
     «превышен предел времени» примерно через 3 с, следующий запрос отправляется. Предупреждение: после такого
     скрипта программа занимает одно ядро процессора (и до 128 МБ памяти), пока её не закрыть; после четырёх
     таких скриптов подряд скрипты перестают исполняться с ошибкой «сбой движка» и текстом `… restart the
     application` — это известное ограничение, после перезапуска всё как обычно.
  7. *Согласие:* импорт чужой коллекции со скриптами → блок «В коллекции есть скрипты» в отчёте; не разрешать →
     Send идёт без скриптов с предупреждением; галочка в редакторе коллекции, раздел Scripts — действует сразу,
     без Ctrl+S; снять галочку; «Выполнять скрипты» в настройках выключает всё; новая коллекция, созданная в
     программе, исполняет скрипты сразу; отчёт импорта называет неподдержанное (`require('moment')`).
  8. *История:* запись показывает список тестов и ошибку скрипта; вывода консоли в записи нет.
  9. *Автодополнение:* в редакторе скрипта после `pm.` — список; `pm.environment.` — методы с подписями;
     Ctrl+Пробел на пустом месте; в теле запроса такого списка нет.
  10. *Экспорт:* после работы скриптов экспорт коллекции и окружения открывается в Postman; значение, записанное
      скриптом (например токен), в файле есть — это известное отличие от Postman.
  11. *Что не работает и чем отличается:* короткий список «позже» из таблицы API; число, положенное в переменную,
      в следующей отправке — строка; `pm.response.status` — стандартная фраза; глобальный `_` — lodash 4; нет
      `Intl`; в test-скрипте `pm.request` — отправленный запрос, но без автоматических заголовков (`User-Agent`,
      `Host` …); значение переменной из скрипта — не длиннее 1 МБ; одиночный суррогат в выводе скрипта
      заменяется знаком «�»; `pm.sendRequest` не подставляет `{{переменные}}` (как в Postman).
  12. *По желанию — семь проверок в живом Postman* (`postman-api.md`, раздел 15) и ещё коллекции для счётчика API.
  13. *Замеры:* две команды из Task 18 и строки таблицы результатов, куда вписать числа, если Step 2 пропущен.
- [ ] **Step 6: Документ результатов** — что сделано по задачам; числа тестов и сборки; замеры Step 2 рядом с
  целями решения № 31; размеры exe и установщика; таблица «API в первой версии» как итог; все решения из раздела
  «Решения автора» с тем, как они реализованы; ответы владельца на четыре вопроса; решения исполнителей там, где
  план молчал; отклонения от плана; новые зависимости (crate и npm) с лицензиями; изменения данных в базе (ключ
  `scripts_allowed`, новые ключи JSON); чем закрыт каждый пункт Review Focus; строка `owner scripts: …`; дефекты,
  найденные набором совместимости; известные ограничения; пометки **[проверить]** (поведение приложения Postman
  по памяти — решения № 9, 10 и часть № 28 про auth и историю; всё, что автотест не видит в живом окне). В
  «известные ограничения» отдельной строкой: скрипт, который движок не может прервать, бросается сторожем, но
  его поток занимает одно ядро процессора и до 128 МиБ памяти до закрытия программы; после
  `MAX_ABANDONED_THREADS` таких потоков скрипты не исполняются до перезапуска (решение № 37). В «изменения данных
  в базе» — и новые значения `error_kind` в `history.summary`.
- [ ] **Step 7: Обновить спеку:** раздел 3 — версии `rquickjs`, chai, lodash; раздел 4 — crate `rqm-script` как
  он устроен, конвейер Send, две новые команды и ссылка на «Контракт команд» этого плана, «всего команд 39»,
  событий нет — консоль приходит с ответом; раздел 5 — ключ `scripts_allowed`, новые ключи JSON в `settings` и
  `history.detail`, новые значения `script` и `skipped` у `error_kind` в `history.summary`, запись переменных из скриптов в `node.content` коллекции, «версия схемы 2 не менялась»;
  раздел 6 — запись отчёта «неподдержанный API», вопрос о согласии как блок отчёта; раздел 8 — рантайм на
  скрипт, байткод один раз за запуск, пределы и их значения (включая пределы хоста и сторож — решения № 37,
  38), `pm.execution.skipRequest` в первой версии (решение № 36, если владелец его не отклонил), запрос в
  test-скрипте и `pm.sendRequest` без подстановки (решения № 40, 28), запись изменений по завершении отправки,
  пометка
  **[проверить]** снята со ссылкой на исходники движка Postman, список API заменён ссылкой на таблицу итогов,
  lodash 4.18.1; раздел 9 — вкладки Tests и Console, галочка согласия и настройки скриптов, абзац «На этапе 1…
  скрипты не выполнены» убрать, фразу «изменение из скрипта не конфликтует с черновиком» исправить (переменные
  коллекции — решение № 14); раздел 10 — строки «запрос с типичным скриптом», «первый запрос со скриптом»,
  размер установщика; раздел 11 — где хранится согласие, пределы, что скрипт может и не может; раздел 12 — этап
  2 выполнен, что отложено; раздел 15 — открытые вопросы.
- [ ] **Step 8: Commit** — «Этап 2: сборка, замеры, список проверки и итоги».
