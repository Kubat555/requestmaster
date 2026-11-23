# RequestMaster — этап 0 «Проверка основы» Implementation Plan

**Goal:** На машине владельца доказать измерениями, что выбранные технологии (Tauri 2 + Svelte 5, rquickjs, reqwest + rustls/ring, rusqlite, serde-модель Postman) собираются под Windows MSVC и укладываются в цели спецификации, — и записать результаты.

**Architecture:** Четыре независимых черновых проекта-пробника в папке `spike/` (скрипты, HTTP, формат Postman, окно), каждый отвечает на свой список вопросов тестами или замерами. Итог этапа — не продукт, а документ с результатами и правки в спецификации. Код из `spike/` в продукт не переносится.

**Tech Stack:** Rust stable (`x86_64-pc-windows-msvc`), MSVC C++ Build Tools + Windows SDK, Node 22, Tauri 2.12.x, Svelte 5, Vite, CodeMirror 6, rquickjs 0.14, reqwest 0.13, rustls (провайдер `ring`), rusqlite (bundled), serde / serde_json (`preserve_order`), esbuild.

**Spec:** `docs/design.md` (разделы 3, 4, 8, 10, 12).

## Global Constraints

- Платформа: Windows 11 x64, таргет `x86_64-pc-windows-msvc`.
- Tauri — только линия 2.12.x; 3.0-alpha не использовать.
- Версии зависимостей закрепляются точно (`=x.y.z` в `Cargo.toml`, точные версии в `package.json`); фактически
  поставленные версии записываются в документ результатов.
- Точные сигнатуры API берутся из docs.rs закреплённой версии, а не по памяти: версии свежие, API мог измениться.
- Криптопровайдер rustls — `ring`; `aws-lc-rs` в дереве зависимостей быть не должно.
- Код в `spike/` — черновой: без обработки всех ошибок, без оформления; в продукт не копируется.
- Git: коммиты и push не делаются без прямой просьбы владельца; `git init` — только с его согласия.
- Приложения и dev-серверы исполнитель сам не запускает (`tauri dev`, `vite dev` — нет). Сборка и `cargo test`
  разрешены. Запуск собранного окна для замеров — владелец либо с его явного разрешения.
- Установка Rust и C++ Build Tools — только после явного разрешения владельца (Task 1).
- Образцы экспортов владельца лежат в `spike/samples/` и никогда не попадают в git (`spike/samples/` в `.gitignore`);
  их содержимое не цитируется в документах — только обезличенная статистика.
- Никаких сетевых обращений, кроме: загрузки зависимостей (crates.io, npm) и одного проверочного HTTPS-запроса
  в Task 3.

## Review Focus

Условия, которые спецификация подразумевает и которые вероятнее всего ударят по пользователю; каждое закреплено
проверкой в указанной задаче.

1. Скрипт с бесконечным циклом или «тяжёлым» регулярным выражением — приложение не зависает, скрипт прерывается
   по лимиту времени (Task 2, тесты `infinite_loop_is_interrupted`, `catastrophic_regex_is_interrupted`).
2. Ответ — минифицированный JSON 10 МБ одной строкой — окно остаётся отзывчивым (Task 5, замер редактора).
3. Внутренний сервис с самоподписанным сертификатом и корпоративный прокси — запрос проходит при выключенной
   проверке SSL; системный прокси определяется (Task 3, тесты `self_signed_passes_when_verification_disabled`,
   `system_proxy_is_detected`).
4. Кириллица: имена запросов, значения переменных, путь профиля пользователя с не-ASCII символами — ничего
   не искажается (Task 4, тест `cyrillic_survives_round_trip`; Task 5, проверка пути к базе).
5. Файл Postman с неизвестными полями и неизвестным типом auth — импорт не падает, поля возвращаются при экспорте
   (Task 4, тест `unknown_fields_and_auth_survive_round_trip`).

---

### Task 1: Инструменты сборки

Выполняется владельцем или исполнителем после явного «да». Требует прав администратора и нескольких ГБ диска.

**Files:**
- Create: `.gitignore` (строки: `target/`, `node_modules/`, `dist/`, `spike/samples/`)
- Create: `spike/README.md` (одна фраза: черновой код этапа 0, в продукт не переносится)

**Interfaces:**
- Produces: рабочие `cargo`, `rustc`, компоновщик MSVC `link.exe`; папка `spike/`.

- [ ] **Step 1: Установить Rust**

```bash
winget install --id Rustlang.Rustup -e
```

Затем в новом терминале: `rustup default stable-x86_64-pc-windows-msvc`.

- [ ] **Step 2: Добавить C++ Build Tools в существующую Visual Studio 17.14**

Через Visual Studio Installer → Modify → рабочая нагрузка «Разработка классических приложений на C++»
(компоненты: MSVC v143 x64/x86 build tools, Windows 11 SDK).

- [ ] **Step 3: Проверить**

Run: `rustc --version; cargo --version`
Expected: версии печатаются, хост `x86_64-pc-windows-msvc` в `rustup show`.

Run: `& "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property displayName`
Expected: непустая строка с названием установленной Visual Studio.

- [ ] **Step 4: Проверить сборку «с нуля»**

Run: `cargo new spike/hello; cargo run --manifest-path spike/hello/Cargo.toml`
Expected: `Hello, world!`. После проверки папку `spike/hello` удалить.

- [ ] **Step 5: `git init`** — только если владелец подтвердил; коммит не делать.

---

### Task 2: Пробник движка скриптов (`spike/script-probe`)

Вопросы: собирается ли rquickjs под MSVC; работают ли chai 4 и lodash; прерываются ли цикл и regex; работает ли
связка Promise ↔ Rust; сколько стоит создание контекста и загрузка прелюдии.

**Files:**
- Create: `spike/script-probe/Cargo.toml` (bin + tests; зависимости: `rquickjs = "=0.14.0"` с нужными features
  для Promise/async по docs.rs, `tokio`)
- Create: `spike/script-probe/js/package.json` (`chai@4.4.1`, `lodash@4.17.21`, `esbuild`), `js/entry.js`
  (кладёт `chai` и `_` в `globalThis`), `js/build.mjs` (esbuild → `js/dist/prelude.js`, формат iife,
  platform neutral)
- Create: `spike/script-probe/src/lib.rs`
- Test: `spike/script-probe/tests/probe.rs`

**Interfaces:**
- Produces (только внутри пробника):
  - `pub struct Limits { pub time: Duration, pub memory_bytes: usize }`
  - `pub enum ScriptOutcome { Ok(String), Threw(String), TimedOut, OutOfMemory }`
  - `pub fn run_script(prelude: &str, source: &str, limits: Limits) -> ScriptOutcome` — свежий Runtime+Context
    на вызов, исполнение в отдельном потоке ОС; результат — JSON-строка значения последнего выражения.
  - `pub fn run_script_async(prelude: &str, source: &str, limits: Limits) -> ScriptOutcome` — то же, плюс
    хост-функция `hostDelay(ms) -> Promise<string>`, которую разрешает Rust (tokio) через канал.

- [ ] **Step 1: Собрать прелюдию**

Run: `npm install --prefix spike/script-probe/js; node spike/script-probe/js/build.mjs`
Expected: файл `spike/script-probe/js/dist/prelude.js` создан; размер записать.

- [ ] **Step 2: Написать падающие тесты**

```rust
const PRELUDE: &str = include_str!("../js/dist/prelude.js");
fn limits() -> Limits { Limits { time: Duration::from_millis(500), memory_bytes: 64 * 1024 * 1024 } }

#[test] fn chai_and_lodash_work() {
    let out = run_script(PRELUDE, "chai.expect(_.sum([1,2])).to.equal(3); 'ok'", limits());
    assert!(matches!(out, ScriptOutcome::Ok(s) if s == "\"ok\""));
}
#[test] fn chai_failure_is_reported() {
    let out = run_script(PRELUDE, "chai.expect(1).to.equal(2)", limits());
    assert!(matches!(out, ScriptOutcome::Threw(m) if m.contains("expected 1 to equal 2")));
}
#[test] fn infinite_loop_is_interrupted() {
    let t = Instant::now();
    assert!(matches!(run_script(PRELUDE, "while(true){}", limits()), ScriptOutcome::TimedOut));
    assert!(t.elapsed() < Duration::from_millis(1500));
}
#[test] fn catastrophic_regex_is_interrupted() {
    let t = Instant::now();
    let out = run_script(PRELUDE, "/(a+)+$/.test('a'.repeat(40)+'b')", limits());
    assert!(matches!(out, ScriptOutcome::TimedOut));
    assert!(t.elapsed() < Duration::from_millis(1500));
}
#[test] fn memory_limit_is_enforced() {
    let out = run_script(PRELUDE, "const a=[]; while(true){a.push('x'.repeat(1<<20))}", limits());
    assert!(matches!(out, ScriptOutcome::OutOfMemory | ScriptOutcome::Threw(_)));
}
#[test] fn promise_resolved_from_rust() {
    let out = run_script_async(PRELUDE, "(async()=>{ return await hostDelay(50) })()", limits());
    assert!(matches!(out, ScriptOutcome::Ok(s) if s == "\"done\""));
}
#[test] fn host_wait_does_not_count_as_script_time() {
    // hostDelay(800) дольше лимита 500 мс: ожидание хоста не должно давать TimedOut
    let out = run_script_async(PRELUDE, "(async()=>{ return await hostDelay(800) })()", limits());
    assert!(matches!(out, ScriptOutcome::Ok(_)));
}
#[test] fn no_ambient_io_globals() {
    let out = run_script(PRELUDE, "[typeof fetch, typeof process, typeof require].join()", limits());
    assert!(matches!(out, ScriptOutcome::Ok(s) if s == "\"undefined,undefined,undefined\""));
}
```

- [ ] **Step 3: Убедиться, что тесты не компилируются/падают**

Run: `cargo test --manifest-path spike/script-probe/Cargo.toml`
Expected: FAIL (функции не определены).

- [ ] **Step 4: Реализовать `run_script` и `run_script_async` в `src/lib.rs`**

Лимит времени — обработчик прерывания движка с дедлайном; для async-варианта дедлайн учитывает только время
исполнения JavaScript (таймер ставится на паузу, пока скрипт ждёт хост). Лимит памяти — настройка Runtime.

- [ ] **Step 5: Прогнать тесты**

Run: `cargo test --manifest-path spike/script-probe/Cargo.toml`
Expected: PASS. Если `catastrophic_regex_is_interrupted` не проходит — тест не «чинить» подгонкой: пометить
`#[ignore]`, записать факт в результаты как открытый риск (лимит на regex потребует другого решения).

- [ ] **Step 6: Замеры (`src/main.rs`)**

Напечатать медиану по 200 повторам: создание Runtime+Context без прелюдии; загрузка прелюдии исходником;
загрузка прелюдии из байткода (если движок это даёт в закреплённой версии).

Run: `cargo run --release --manifest-path spike/script-probe/Cargo.toml`
Expected: три числа в микросекундах; записать в результаты. Решение для спецификации: если загрузка исходником
≤ 5 мс — байткод не нужен.

---

### Task 3: Пробник HTTP (`spike/http-probe`)

Вопросы: собирается ли reqwest 0.13 с rustls + `ring` без `aws-lc-rs`; хватает ли его возможностей для раздела 7
спецификации.

**Files:**
- Create: `spike/http-probe/Cargo.toml` (`reqwest = "=0.13.x"` без default-features, с features для rustls без
  встроенного провайдера, gzip, brotli, deflate, zstd, stream, multipart, cookies, system-proxy — точные имена по
  docs.rs; `rustls` с feature `ring`; `tokio`, `tokio-util`; dev: `wiremock`, `rcgen` для самоподписанного
  сертификата и локального TLS-сервера)
- Create: `spike/http-probe/src/lib.rs`
- Test: `spike/http-probe/tests/probe.rs`

**Interfaces:**
- Produces (только внутри пробника):
  - `pub struct SendOptions { pub follow_redirects: bool, pub max_redirects: u32, pub verify_tls: bool, pub timeout: Duration }`
  - `pub struct Hop { pub status: u16, pub url: String }`
  - `pub struct Outcome { pub status: u16, pub hops: Vec<Hop>, pub ttfb: Duration, pub total: Duration, pub wire_bytes: u64, pub body: Vec<u8> }`
  - `pub async fn send(method: &str, url: &str, headers: &[(String, String)], body: Option<Vec<u8>>, opts: &SendOptions, cancel: CancellationToken) -> Result<Outcome, SendError>`
  - `pub enum SendError { Cancelled, Timeout, Tls(String), Other(String) }`
  - `pub fn system_proxy() -> Option<String>`

- [ ] **Step 1: Проверить дерево зависимостей**

Run: `cargo build --manifest-path spike/http-probe/Cargo.toml; cargo tree --manifest-path spike/http-probe/Cargo.toml -i aws-lc-rs`
Expected: сборка успешна; вторая команда сообщает, что пакет `aws-lc-rs` не найден.

- [ ] **Step 2: Написать падающие тесты** (сервер — `wiremock`, кроме TLS-тестов)

```rust
#[tokio::test] async fn redirect_chain_is_captured() {
    // /a -> 302 /b -> 301 /c -> 200 "end"
    let out = send("GET", &url("/a"), &[], None, &opts(true), CancellationToken::new()).await.unwrap();
    assert_eq!(out.status, 200);
    assert_eq!(out.hops.iter().map(|h| h.status).collect::<Vec<_>>(), vec![302, 301]);
}
#[tokio::test] async fn redirects_not_followed_when_disabled() {
    let out = send("GET", &url("/a"), &[], None, &opts(false), CancellationToken::new()).await.unwrap();
    assert_eq!(out.status, 302);
    assert!(out.hops.is_empty());
}
#[tokio::test] async fn gzip_body_is_decoded_and_wire_size_reported() {
    // сервер отдаёт 100 КБ нулей, сжатых gzip
    let out = send("GET", &url("/gz"), &[], None, &opts(true), CancellationToken::new()).await.unwrap();
    assert_eq!(out.body.len(), 100 * 1024);
    assert!(out.wire_bytes < 10 * 1024);
}
#[tokio::test] async fn cancel_stops_slow_request() {
    // сервер отвечает через 5 с; отмена через 100 мс
    let t = Instant::now();
    let r = send("GET", &url("/slow"), &[], None, &opts(true), cancel_after(100)).await;
    assert!(matches!(r, Err(SendError::Cancelled)));
    assert!(t.elapsed() < Duration::from_secs(1));
}
#[tokio::test] async fn ttfb_is_less_than_total_for_delayed_body() {
    let out = send("GET", &url("/drip"), &[], None, &opts(true), CancellationToken::new()).await.unwrap();
    assert!(out.ttfb < out.total);
}
#[tokio::test] async fn self_signed_fails_by_default() {
    let r = send("GET", &tls_url("/"), &[], None, &opts(true), CancellationToken::new()).await;
    assert!(matches!(r, Err(SendError::Tls(_))));
}
#[tokio::test] async fn self_signed_passes_when_verification_disabled() {
    let o = SendOptions { verify_tls: false, ..opts(true) };
    assert_eq!(send("GET", &tls_url("/"), &[], None, &o, CancellationToken::new()).await.unwrap().status, 200);
}
#[test] fn system_proxy_is_detected() {
    // не утверждение о значении: функция не паникует и возвращает то же, что настройки Windows
    let _ = system_proxy();
}
```

- [ ] **Step 3: Убедиться, что тесты падают**

Run: `cargo test --manifest-path spike/http-probe/Cargo.toml`
Expected: FAIL (функции не определены).

- [ ] **Step 4: Реализовать `send` и `system_proxy` в `src/lib.rs`**

Авто-redirect reqwest выключен, цикл редиректов свой. Декомпрессия: если reqwest в закреплённой версии не отдаёт
размер «на проводе» при авто-декомпрессии — отключить её и декодировать самим; какой путь выбран, записать
в результаты. `verify_tls: false` требует отдельного клиента — клиенты кэшировать по ключу настроек.

- [ ] **Step 5: Прогнать тесты**

Run: `cargo test --manifest-path spike/http-probe/Cargo.toml`
Expected: PASS.

- [ ] **Step 6: Один настоящий HTTPS-запрос и системный прокси**

Run: `cargo run --manifest-path spike/http-probe/Cargo.toml -- https://example.com`
Expected: статус 200, печать `ttfb`, `total`, значения `system_proxy()`. Сверить вывод прокси с настройками
Windows владельца; результат записать. Проверка цепочки через системное хранилище сертификатов считается
подтверждённой, если запрос прошёл с `verify_tls: true`.

---

### Task 4: Пробник формата Postman (`spike/postman-probe`)

Вопросы: сохраняет ли модель «типизированное ядро + мешок неизвестных полей» данные при цикле импорт → экспорт;
какие `pm.*` и `require` реально используются в коллекциях владельца; как устроен zip-дамп.

**Files:**
- Create: `spike/postman-probe/Cargo.toml` (`serde`, `serde_json` с feature `preserve_order`; без
  `arbitrary_precision`; `regex`, `walkdir`)
- Create: `spike/postman-probe/src/lib.rs` (минимальные структуры v2.1: Collection, Info, Item — папка или запрос
  по наличию ключа `item`/`request`, Request строкой или объектом, Url строкой или объектом, Header, Body, Auth,
  Event, Script с `exec` строкой или массивом, Variable; на каждой структуре поле
  `#[serde(flatten)] extra: serde_json::Map<String, Value>`)
- Create: `spike/postman-probe/src/main.rs` (обход `spike/samples/`, отчёт)
- Create: `spike/postman-probe/fixtures/synthetic.postman_collection.json` — собственный образец: 2 уровня папок;
  запрос с `url` строкой и запрос с `url` объектом; `exec` строкой и массивом; auth `basic`, отсутствующий auth
  (наследование), `noauth` и выдуманный тип `"x-future"`; неизвестные ключи на каждом уровне (`_exporter_id`,
  `protocolProfileBehavior`, `x-unknown`); body `raw`, `urlencoded`, `formdata` с файлом; имена и значения
  на кириллице; сохранённый пример ответа
- Create: `spike/postman-probe/fixtures/synthetic.postman_environment.json` (значения `default`, `secret`,
  отключённая переменная)
- Test: `spike/postman-probe/tests/roundtrip.rs`

**Interfaces:**
- Produces (только внутри пробника):
  - `pub fn parse_collection(json: &str) -> Result<Collection, serde_json::Error>`
  - `pub fn to_json(c: &Collection) -> String`
  - `pub fn semantic_diff(a: &serde_json::Value, b: &serde_json::Value) -> Vec<String>` — пути расхождений;
    порядок ключей объекта игнорируется, порядок элементов массива — нет
  - `pub fn script_api_usage(c: &Collection) -> BTreeMap<String, usize>` — счётчики вхождений `pm.<a>.<b>`,
    `postman.<a>`, `require('<x>')`, а также `tests[`, `responseBody`, `responseCode`

- [ ] **Step 1: Написать падающие тесты**

```rust
fn fixture() -> String { std::fs::read_to_string("fixtures/synthetic.postman_collection.json").unwrap() }

#[test] fn synthetic_round_trip_has_empty_diff() {
    let src = fixture();
    let out = to_json(&parse_collection(&src).unwrap());
    let d = semantic_diff(&serde_json::from_str(&src).unwrap(), &serde_json::from_str(&out).unwrap());
    assert!(d.is_empty(), "{d:#?}");
}
#[test] fn unknown_fields_and_auth_survive_round_trip() {
    let out = to_json(&parse_collection(&fixture()).unwrap());
    for needle in ["_exporter_id", "x-unknown", "x-future", "protocolProfileBehavior"] {
        assert!(out.contains(needle), "{needle} lost");
    }
}
#[test] fn cyrillic_survives_round_trip() {
    let out = to_json(&parse_collection(&fixture()).unwrap());
    assert!(out.contains("Получить токен"));
}
#[test] fn folder_and_request_are_distinguished() {
    let c = parse_collection(&fixture()).unwrap();
    assert_eq!(count_folders(&c), 2);
    assert_eq!(count_requests(&c), 4);
}
#[test] fn script_api_usage_counts_calls() {
    let u = script_api_usage(&parse_collection(&fixture()).unwrap());
    assert_eq!(u.get("pm.environment.set"), Some(&1));
    assert_eq!(u.get("require('lodash')"), Some(&1));
}
#[test] fn all_owner_samples_round_trip() {
    // для каждого *.json в ../samples с info.schema v2.1: diff пуст; если папки нет — тест пропускается с сообщением
}
```

Образец обязан содержать один вызов `pm.environment.set`, один `require('lodash')`, запрос с именем
«Получить токен», ровно 2 папки и 4 запроса.

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `cargo test --manifest-path spike/postman-probe/Cargo.toml`
Expected: FAIL.

- [ ] **Step 3: Реализовать структуры и функции в `src/lib.rs`**

Полиморфные места — enum с двумя вариантами, сохраняющий исходную форму (строка или объект/массив).

- [ ] **Step 4: Прогнать тесты**

Run: `cargo test --manifest-path spike/postman-probe/Cargo.toml`
Expected: PASS. Любое расхождение на образцах владельца — не обходить, а записать путь расхождения в результаты
(без значений).

- [ ] **Step 5: Отчёт по образцам владельца (`src/main.rs`)**

Run: `cargo run --manifest-path spike/postman-probe/Cargo.toml -- spike/samples`
Expected: таблица: число коллекций, папок, запросов; версии `info.schema`; встреченные типы auth, режимы body,
ключи `protocolProfileBehavior`; сводные счётчики `script_api_usage`; для zip-дампа — список путей внутри архива
(только имена папок и расширения). Таблицу перенести в результаты. Если образцов нет — зафиксировать это как
невыполненную часть этапа.

---

### Task 5: Пробник окна (`spike/shell`)

Вопросы: холодный запуск и память Tauri на этой машине; порог размера ответа для CodeMirror; цена передачи тела
из ядра в интерфейс; линкуется ли rusqlite bundled; размер установщика.

**Files:**
- Create: `spike/shell/` — проект Tauri 2.12.x + Svelte 5 + TypeScript + Vite (шаблон `create-tauri-app`,
  системный заголовок окна, без прозрачности)
- Create: `spike/shell/src-tauri/src/lib.rs` — команды:
  - `gen_json(mb: u32, minified: bool) -> String` — синтетический JSON заданного размера (массив объектов);
  - `gen_bytes(mb: u32) -> tauri::ipc::Response` — те же данные сырыми байтами;
  - `db_roundtrip() -> String` — открыть SQLite в каталоге данных приложения, создать временную таблицу,
    записать и прочитать строку с кириллицей, вернуть прочитанное;
  - `mark_ready()` — записать в `%TEMP%\requestmaster-spike-ready.txt` время в мс с момента старта процесса.
- Create: `spike/shell/src/App.svelte` — после первой отрисовки вызывает `mark_ready`; панель замеров: кнопки
  размеров 1 / 5 / 10 / 20 МБ × (pretty | minified) × (строка | байты); показывает время передачи и время
  до готовности CodeMirror; поле ввода для проверки отзывчивости при открытом большом документе
- Create: `spike/shell/measure-start.ps1` — 10 раз: запустить собранный exe, дождаться файла готовности,
  прочитать время, просуммировать рабочую память процесса и дочерних `msedgewebview2.exe`, закрыть; напечатать
  медиану и максимум

**Interfaces:**
- Produces: собранный `spike/shell/src-tauri/target/release/*.exe` и установщик; числа для раздела 10 спецификации.

- [ ] **Step 1: Собрать проект**

Run: `npm install --prefix spike/shell; npm run tauri build --prefix spike/shell`
Expected: сборка успешна; записать размер exe и установщика (цель спецификации: установщик ≤ 20 МБ).

- [ ] **Step 2: Замер запуска и памяти** (запускает владелец либо исполнитель с его разрешения)

Run: `powershell -File spike/shell/measure-start.ps1`
Expected: медиана холодного запуска (цель ≤ 1000 мс) и суммарной памяти; записать оба числа, а также результат
первого запуска после перезагрузки отдельно.

- [ ] **Step 3: Замеры в окне** (владелец либо с его разрешения)

Пройти матрицу размеров в панели замеров. Для каждой ячейки записать: время передачи, время до готовности
редактора, остаётся ли ввод отзывчивым (да/нет). Проверить `db_roundtrip`: возвращается строка с кириллицей
без искажений, файл базы лежит в `%LOCALAPPDATA%`.

- [ ] **Step 4: Вывести порог**

Порог «показывать целиком» = наибольший размер, при котором и pretty, и minified дают готовность редактора
≤ 300 мс и отзывчивый ввод. Способ передачи = тот, что быстрее на этом пороге. Записать оба решения.

---

### Task 6: Результаты и правка спецификации

**Files:**
- Create: `stage0-findings.md`
- Modify: `docs/design.md` (разделы 3, 4, 8, 10; пометки **[проверить]**)

- [ ] **Step 1: Написать документ результатов**

Разделы: фактические версии инструментов и зависимостей; по каждому пробнику — вопрос → ответ → число или факт;
таблица замеров против целей раздела 10; список использованных владельцем `pm.*` и `require` с выводом, что
поднять в первую версию; структура zip-дампа; открытые риски (всё, что не прошло или не проверено).

- [ ] **Step 2: Обновить спецификацию**

Каждую пометку **[проверить]** заменить на подтверждённый факт либо на изменённое решение со ссылкой на документ
результатов; в разделе 10 цели заменить на измеренные исходные значения + цель; порог большого ответа и способ
передачи — по Task 5; список API скриптов первой версии — по Task 4.

- [ ] **Step 3: Вынести решения владельцу**

Отдельным списком: всё, где измерение не уложилось в цель или выбор технологии пришлось менять. Изменение
технологии из раздела 3 без согласия владельца не вносится.

- [ ] **Step 4: Папка `spike/`**

Спросить владельца: удалить сразу или оставить до старта этапа 1. Не удалять без ответа.

---

## Порядок и зависимости

Task 1 — первым. Task 2, 3, 4, 5 друг от друга не зависят и могут идти параллельно. Task 6 — последним.
Task 4 полностью выполним только при наличии образцов владельца в `spike/samples/`; без них выполняется часть
на синтетическом образце.
