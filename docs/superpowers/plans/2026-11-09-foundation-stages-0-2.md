# Subversum Rust+WASM: фундамент (этапы 0–2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Каркас репозитория, пакет `versum-dsp` v0.1.0 и работающий скелет движка (Rust → WASM → AudioWorklet) с протоколом команд и телеметрии и тестовым миром-пробой на странице `/next/`.

**Architecture:** DSP-блоки живут в отдельном репозитории `versum-dsp` и подключаются git-зависимостью по тегу. В `subversum` — Cargo workspace (`engine` с протоколом, миром-пробой и тестами; `wasm` с `extern "C"`) и Vite+TS в `web/`. Старое приложение переезжает в `legacy/` и на деплое копируется в корень; новое собирается в `/next/`.

**Tech Stack:** Rust (stable 1.99, таргет `wasm32-unknown-unknown`, без `wasm-bindgen`), Vite + TypeScript + Vitest, AudioWorklet, Vercel.

**Spec:** `docs/superpowers/specs/2026-11-09-rust-wasm-refactor-design.md`

**Границы плана.** Покрывает этапы 0, 1, 2 спеки. Этап 3 (мир amber) — отдельный план, он пишется после приёмки этого. Отклонения от спеки, принятые здесь сознательно: (а) `Telemetry` пока без энергии по 8 диапазонам (`bands`) — поле появится в плане этапа 3 вместе с первым миром, которому оно нужно, версия протокола при этом вырастет; (б) `Delay` владеет своим буфером, выделенным в `new` (а не принимает внешний): это соблюдает правило «аллокация только в `new`».

## Global Constraints

- Аудио в `f32`, фазы и накопители в `f64`; после `new()` блоки не аллоцируют и не блокируют.
- Контракт блока: `new`, `process`/`tick`, `reset()`, NaN-guard на выходе.
- Блок звука — 128 сэмплов (`MAX_BLOCK = 128`).
- WASM: `cargo build --release --target wasm32-unknown-unknown`, без `wasm-bindgen`, интерфейс `extern "C"` и линейная память.
- Транспорт: `postMessage` через порт AudioWorklet, телеметрия батчем раз в кадр (около 60 Гц); SharedArrayBuffer и COOP/COEP не используются.
- Зависимость: на время разработки `versum-dsp = { git = "https://github.com/ai-slopes-hq/versum-dsp", rev = "<sha коммита из ветки feat/foundation>" }`; в финале, после merge ветки в `main` репозитория `versum-dsp` и тега, `rev` заменяется на `tag = "v0.1.0"`.
- Деплой: `legacy/` копируется в корень `dist/` как есть, новое приложение лежит в `dist/next/`; в git только исходники; Rust и Vite собираются на Vercel.
- PWA-кэш legacy: `subversum-v3`.
- `?v=1` принадлежит legacy, новый движок использует `?v=2`.
- Текст интерфейса и сообщения коммитов — как в существующем репозитории (коммиты по-английски, по делу).

## Review Focus

1. Старый `sw.js` на корневом scope перехватывает `/next/`: отдаёт закэшированные js/wasm cache-first и записывает HTML `/next/` поверх закэшированного `index.html` корня. Ожидание: `/next/*` он не трогает. Тест — Task 3.
2. `engine_process(frames)` с `frames` равным 0 или больше 128 не паникует и не выходит за буфер. Ожидание: ограничивает до `MAX_BLOCK`, 0 — ничего не делает. Тест — Task 5.
3. `engine_init` вызывается повторно (worklet пересоздан после `resume`). Ожидание: память не растёт, состояние сброшено. Тест — Task 5.
4. Больше 64 команд за блок. Ожидание: лишние отбрасываются со счётчиком, без паники. Тест — Task 4.
5. Больше 16 шагов между публикациями телеметрии (вкладка в фоне). Ожидание: счётчик `dropped` растёт, новейшие шаги не теряются молча. Тест — Task 4.

---

## Файлы

`versum-dsp` (`/Users/noise/code/versum-dsp`):
- `Cargo.toml`, `src/lib.rs` (реэкспорт), `src/block.rs` (`Block`), `src/rng.rs`, `src/dc.rs`, `src/onepole.rs`, `src/ramp.rs`, `src/knee.rs`, `src/pan.rs`, `src/edge.rs`, `src/delay.rs`, `src/testkit.rs` (за фичей `testkit`), `tests/*.rs`.

`subversum`:
- `legacy/` — `index.html`, `sw.js`, `manifest.webmanifest`, `icons/`, `lab/` (перенос); `legacy/sw.test.mjs`.
- `Cargo.toml` (workspace), `crates/engine/` (`src/lib.rs`, `protocol.rs`, `engine.rs`, `probe.rs`, `peaks.rs`, `tests/`), `crates/wasm/` (`src/lib.rs`, `tests/`).
- `package.json`, `tsconfig.json`, `vite.config.ts`, `web/index.html`, `web/audio/{protocol,host,worklet}.ts`, `web/gfx/{skin,debug-skin,synthetic}.ts`, `web/input/gesture.ts`, `web/app/main.ts`, `web/**/*.test.ts`.
- `scripts/build.sh`, `scripts/vercel-install.sh`, `vercel.json`, `.gitignore`.

---

### Task 1: Каркас `versum-dsp`, `Block`, `testkit`, `Rng`

Риск: обычный — чистые функции и тестовая обвязка, без realtime и unsafe в продукте.

**Files:**
- Create: `/Users/noise/code/versum-dsp/{Cargo.toml,src/lib.rs,src/block.rs,src/rng.rs,src/testkit.rs,tests/rng.rs,tests/testkit.rs}`

**Interfaces:**
- Produces:
  - `pub trait Block { fn tick(&mut self, x: f32) -> f32; fn reset(&mut self); fn process(&mut self, buf: &mut [f32]) { /* tick по каждому сэмплу */ } }`
  - `pub struct Rng; impl Rng { pub fn new(seed: u32) -> Rng; pub fn next_u32(&mut self) -> u32; pub fn next_f64(&mut self) -> f64 }` — `next_f64` равен JS `mulberry32` (`index.html:574`) бит в бит.
  - За фичей `testkit`: `pub struct CountingAlloc` (реализует `GlobalAlloc`, считает `alloc` по потокам), `pub fn count_allocs<R>(f: impl FnOnce() -> R) -> (R, usize)`, `pub fn assert_blockwise_eq<B: Block>(make: impl Fn() -> B, input: &[f32], chunks: &[usize])`.

- [ ] **Step 1: Инициализировать крейт** в существующем репозитории: `Cargo.toml` с `name = "versum-dsp"`, `edition = "2021"`, фича `testkit = []`; `.gitignore` с `target/`.
- [ ] **Step 2: Failing-тест `tests/rng.rs`.** Сгенерировать эталон командой `node -e` с функцией из `index.html:574`: по 8 значений для сидов `0`, `1`, `0xFFFFFFFF`, `0xDEADBEEF`. В тесте `rng_matches_js_mulberry32`: для каждого сида `Rng::new(seed).next_f64()` равен вставленному эталону (сравнение `==`, не `abs < eps`). Плюс `rng_u32_is_float_times_2_pow_32`: `next_f64() * 4294967296.0` равно `next_u32()` для одного сида.
- [ ] **Step 3: Запустить** `cargo test --test rng` → FAIL (нет `Rng`).
- [ ] **Step 4: Реализовать `Rng`** (шаг `0x6D2B79F5` на `u32` с `wrapping_*`; делитель `4294967296.0`) и `Block` в `src/block.rs`.
- [ ] **Step 5: Запустить** `cargo test --test rng` → PASS.
- [ ] **Step 6: Failing-тест `tests/testkit.rs`** (`--features testkit`, свой `#[global_allocator]` из `CountingAlloc`): `count_allocs_counts_vec_new` — `count_allocs(|| Vec::<u8>::with_capacity(8)).1 == 1`; `count_allocs_zero_for_pure_math` — `.1 == 0`; `blockwise_helper_detects_bad_override` — тестовый `Block`, чей `process` ведёт себя иначе, чем `tick` по сэмплу, заставляет `assert_blockwise_eq` паниковать (`#[should_panic]`).
- [ ] **Step 7: Реализовать `src/testkit.rs`**; `count_allocs` читает потоковый счётчик до и после. Запустить `cargo test --features testkit` → PASS.
- [ ] **Step 8: Commit** в `versum-dsp`: `git add -A && git commit -m "feat: Block trait, Rng (mulberry32, matches JS), testkit"`.

---

### Task 2: Примитивы `versum-dsp` (ветка `feat/foundation`)

Риск: обычный — чистая математика под тестами из каталога DSP.

**Files:**
- Create: `src/{dc,onepole,ramp,knee,pan,edge,delay}.rs`, `tests/primitives.rs`; Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `Block`, `testkit` из Task 1.
- Produces:
  - `DcBlock::new(hz: f32, sr: f32) -> DcBlock` (`R = exp(-2π·hz/sr)`), `impl Block`.
  - `OnePole::new(hz: f32, sr: f32)`, `set_hz(&mut self, hz: f32)`, `tick2(&mut self, x: f32) -> (f32, f32)` (LP, HP), `impl Block` (выход — LP).
  - `Ramp::new(sr: f32, ms: f32)`, `set_target(&mut self, v: f32)`, `next(&mut self) -> f32` (линейно, садится на цель ровно за `ms`), `value(&self) -> f32`.
  - `Knee::new(k: f32, kn: f32, room: f32)`, `impl Block` (`y = k·x`; при `|y| > kn` выход `sign·(kn + room·tanh((|y|−kn)/room))`).
  - `pan_eq(pan: f32) -> (f32, f32)` (`pan` в [−1, 1], θ = (pan+1)·π/4, возвращает (cos θ, sin θ)).
  - `EdgeDetector::new()`, `tick(&mut self, x: f32) -> bool` (фронт при `x > 0.5`, перевзвод при `x < 0.25`).
  - `Delay::new(max_samples: usize)`, `write_read(&mut self, x: f32, delay: f32) -> f32` (линейная интерполяция, `delay` ≥ 1, значения вне диапазона зажимаются), `reset()`.

- [ ] **Step 1: Failing-тесты в `tests/primitives.rs`** (по именам из каталога DSP):
  - `dc_decays_below_one_percent`: вход 1.0 при `DcBlock::new(5.0, 48000.0)`, после 12000 сэмплов `|y| < 0.01`.
  - `knee_is_linear_below_and_bounded_above`: `Knee::new(1.0, 0.9, 0.1)`: `tick(0.5) == 0.5`; `tick(100.0) <= 1.0`; нечётность `tick(-x) == -tick(x)`; монотонность на сетке от 0 до 10.
  - `pan_is_equal_power`: для сетки `pan` сумма квадратов равна 1 с допуском 1e-6; `pan_eq(-1.0) ≈ (1, 0)`, `pan_eq(1.0) ≈ (0, 1)`.
  - `an_edge_fires_once_and_rearms_below_a_quarter`: последовательность 0, 0.6, 0.6, 0.4, 0.6 даёт одно срабатывание; после 0.2 — второе.
  - `ramp_lands_exactly_on_target`: `Ramp::new(48000.0, 10.0)` от 0 к 1 за 480 вызовов `next`, затем `value() == 1.0` ровно.
  - `onepole_lp_plus_hp_equals_input`: для шума `lp + hp` равно входу с допуском 1e-5.
  - `delay_reads_back_integer_and_fractional`: импульс на входе, чтение с `delay = 4.0` даёт импульс через 4 сэмпла, с `4.5` — по 0.5 в двух соседних.
  - `blocks_are_blockwise_equal`: `assert_blockwise_eq` для `DcBlock`, `OnePole`, `Knee` на 4096 сэмплах шума, чанки `[1, 7, 64, 128, 3]`.
  - `no_alloc_after_new`: `count_allocs` вокруг `process` каждого блока и `Delay::write_read` равен 0.
  - `outputs_are_finite_on_nan_input`: подать `f32::NAN` и `f32::INFINITY`; следующие 100 выходов конечны (NaN-guard с `reset()`).
- [ ] **Step 2: Запустить** `cargo test --features testkit` → FAIL.
- [ ] **Step 3: Реализовать блоки** по сигнатурам выше; NaN-guard: если выход неконечный — обнулить выход и вызвать `reset()`.
- [ ] **Step 4: Запустить** `cargo test --features testkit` → PASS.
- [ ] **Step 5: Commit** `feat: DcBlock, OnePole, Ramp, Knee, pan_eq, EdgeDetector, Delay`.
- [ ] **Step 6: Работа ведётся в ветке `feat/foundation`** репозитория `versum-dsp` (создать её до Task 1; `main` не трогать). Тег не ставится до приёмки.
- [ ] **Step 7 (с подтверждения пользователя): выложить ветку** `git push -u origin feat/foundation` в `ai-slopes-hq/versum-dsp`. Затем в Task 4 зависимость в `crates/engine/Cargo.toml` пинуется на `rev` последнего коммита ветки; обновлять `rev` нужно при каждом новом коммите `versum-dsp`. До push допустим `[patch]` на `/Users/noise/code/versum-dsp` в незакоммиченном `.cargo/config.toml`.

---

### Task 3: Перенос в `legacy/`, обход `/next/` в service worker, каркас workspace

Риск: высокий — меняет `sw.js`, который уже стоит у реальных пользователей PWA; ошибка ломает офлайн-кэш прода.

**Files:**
- Move: `index.html`, `sw.js`, `manifest.webmanifest`, `icons/`, `lab/` → `legacy/` (`git mv`)
- Modify: `legacy/sw.js`
- Create: `legacy/sw.test.mjs`, `Cargo.toml` (workspace, members `crates/*`), `.gitignore` (`target/`, `node_modules/`, `dist/`, `web/public/engine.wasm`)

**Interfaces:**
- Produces: `legacy/sw.js` не вмешивается в запросы с путём, начинающимся с `/next/`; кэш `subversum-v3`.

- [ ] **Step 1: Failing-тест `legacy/sw.test.mjs`** (`node --test`): загрузить `sw.js` в `vm` с заглушками `self` (сбор слушателей `fetch`), `caches`, `location.origin = 'https://x.test'`, `URL`. Тесты:
  - `next_navigation_is_not_intercepted`: событие `fetch` для `GET https://x.test/next/` с `mode: 'navigate'`; `respondWith` не вызван.
  - `next_assets_are_not_intercepted`: то же для `/next/assets/a.js` и `/next/engine.wasm`.
  - `root_navigation_still_handled`: для `/` `respondWith` вызван.
  - `cache_name_is_v3`: заглушка `caches.open` получает `'subversum-v3'` при событии `install`.
- [ ] **Step 2: `git mv`** перечисленных файлов в `legacy/`; запустить `node --test legacy/sw.test.mjs` → FAIL.
- [ ] **Step 3: В `legacy/sw.js`** сменить `CACHE` на `subversum-v3` и добавить ранний `return` в обработчике `fetch` для `url.pathname.startsWith('/next/')`, рядом с существующим пропуском `/_vercel/`.
- [ ] **Step 4: Запустить** `node --test legacy/sw.test.mjs` → PASS.
- [ ] **Step 5: Проверить, что legacy работает после переноса:** `python3 -m http.server 8765 -d legacy`, открыть `http://localhost:8765/`, убедиться, что страница грузится (иконки и манифест доступны, ошибок 404 в сети нет).
- [ ] **Step 6: Создать корневой `Cargo.toml`** (`[workspace] members = ["crates/*"]`, `resolver = "2"`) и `.gitignore`.
- [ ] **Step 7: Commit** `refactor: move app to legacy/, keep service worker out of /next/, cargo workspace`.

---

### Task 4: `engine` — протокол, `World`, `Engine`, мир-проба

Риск: высокий — realtime-аудио, отсутствие аллокаций на аудиопотоке, фиксированные очереди.

**Files:**
- Create: `crates/engine/{Cargo.toml,src/lib.rs,src/protocol.rs,src/engine.rs,src/probe.rs,src/peaks.rs}`, `crates/engine/tests/engine.rs`, `crates/engine/examples/render.rs`

**Interfaces:**
- Consumes: `versum-dsp` (git, `rev` коммита из ветки `feat/foundation`; фича `testkit` в `dev-dependencies`): `Block`, `Rng`, `DcBlock`, `Knee`, `Ramp`.
- Produces (`protocol.rs`, всё `#[repr(C)]`, `Copy`):
  - `pub const MAX_BLOCK: usize = 128; pub const CMD_QUEUE: usize = 64; pub const STEP_QUEUE: usize = 16; pub const PROTOCOL_VERSION: u32 = 1;`
  - `pub struct Gesture { pub x: f32, pub y: f32, pub z: f32, pub pinch: f32 }`
  - `pub enum Command { Gesture(Gesture), SwitchWorld(u32), Seed(u32), Tempo(f32), Hold(bool), Lock(bool) }`
  - `pub struct StepEvent { pub index: u32, pub note: u32, pub velocity: f32 }`
  - `pub struct Telemetry { pub version: u32, pub frame: u32, pub peak_l: f32, pub peak_r: f32, pub step_count: u32, pub dropped_steps: u32, pub dropped_commands: u32, pub steps: [StepEvent; STEP_QUEUE] }`
  - `pub trait World { fn on_gesture(&mut self, g: &Gesture); fn on_step(&mut self, s: &StepEvent); fn render(&mut self, l: &mut [f32], r: &mut [f32]); fn reset(&mut self, seed: u32); }`
- Produces (`engine.rs`): `Engine::new(sr: f32, seed: u32) -> Engine`; `push(&mut self, c: Command)`; `process(&mut self, l: &mut [f32], r: &mut [f32])` (длина ≤ `MAX_BLOCK`, одинаковая); `telemetry(&self) -> &Telemetry`; `ack_steps(&mut self)` (обнуляет `step_count`, `dropped_steps`).
- Производящий мир `ProbeWorld`: синус, частота зависит от `Gesture.x` (110–880 Гц, экспонента), громкость от `y`; на каждый `on_step` запускается короткий блип (затухающий синус 1 кГц); шаги генерирует `Engine` каждые `60/bpm/4` секунды (шестнадцатые), `Tempo` по умолчанию 120.

- [ ] **Step 1: Failing-тесты `tests/engine.rs`** (`--features` не нужны, allocator задаётся в самом файле через `versum_dsp::testkit`):
  - `blockwise_equals_per_sample`: движок с одним сидом, `process` блоками `[128]` против блоков `[1]`; левые и правые выходы побитово равны.
  - `no_alloc_in_process`: `count_allocs` вокруг 1000 вызовов `process` блоками по 128 с командами `Gesture` между ними — 0.
  - `step_lands_on_exact_sample`: `sr = 48000`, `Tempo(120)`; второй шаг приходит ровно на сэмпле 6000: `telemetry().steps` содержит `index == 1` после блока, покрывающего его, и блип в выходе начинается на нужном сэмпле (первый ненулевой отсчёт блипа после тишины — индекс 6000).
  - `outputs_finite_and_bounded`: 10 секунд при случайных `Gesture`, `|y| <= 1.0` и ни одного NaN; пики в телеметрии ≤ 1.
  - `command_queue_overflow_is_counted`: 100 `push` за один блок → `dropped_commands == 36`, паники нет.
  - `step_queue_overflow_keeps_newest_and_counts`: 20 шагов без `ack_steps` → `step_count == 16`, `dropped_steps == 4`, последний элемент очереди — шаг с наибольшим `index`.
  - `peaks_decay_after_silence`: после остановки проб-тона `peak_l` падает ниже 1% за 300 мс × 5.
  - `switch_world_and_seed_reset_state`: `Seed(1)` затем `Seed(2)` меняют выход; повторный `Seed(1)` воспроизводит первый выход побитово.
- [ ] **Step 2: Запустить** `cargo test -p engine` → FAIL.
- [ ] **Step 3: Реализовать `protocol.rs`, `peaks.rs`** (спад 300 мс, как в `ST`), **`probe.rs`, `engine.rs`.** Очередь команд — массив на стеке; блок делится на куски по границам шагов, шаг вызывает `World::on_step` ровно на своём сэмпле. Выходной тракт: `Knee::new(1.0, 0.9, 0.1)` затем `DcBlock::new(5.0, sr)`.
- [ ] **Step 4: Запустить** `cargo test -p engine` → PASS.
- [ ] **Step 5: `examples/render.rs`:** читает `--seconds`, `--seed`, `--out`, пишет 24-битный WAV (без новых зависимостей — заголовок вручную). Запустить `cargo run -p engine --example render -- --seconds 4 --seed 1 --out <файл в scratchpad>` и проверить размер файла (`4·48000·2·3 + 44` байт).
- [ ] **Step 6: Commit** `feat(engine): protocol, Engine, probe world, native tests and WAV render`.

---

### Task 5: `wasm` — экспорт `extern "C"`

Риск: высокий — `unsafe`, статическое состояние, линейная память, realtime.

**Files:**
- Create: `crates/wasm/{Cargo.toml (name = "versum-wasm"),src/lib.rs}`, `crates/wasm/tests/exports.rs`

**Interfaces:**
- Consumes: `engine::{Engine, Command, Gesture, MAX_BLOCK, Telemetry}`.
- Produces (имена экспортов wasm, `crate-type = ["cdylib", "rlib"]`, имя артефакта `versum_wasm.wasm`):
  - `engine_init(sample_rate: f32, seed: u32)`
  - `engine_command(kind: u32, a: f32, b: f32, c: f32, d: f32)` — `kind`: 0 Gesture(x,y,z,pinch), 1 SwitchWorld(a), 2 Seed(a), 3 Tempo(a), 4 Hold(a≠0), 5 Lock(a≠0)
  - `engine_process(frames: u32)`
  - `engine_out_l() -> *const f32`, `engine_out_r() -> *const f32`
  - `engine_telemetry() -> *const u8`, `engine_telemetry_size() -> u32`, `engine_ack() -> ()`

- [ ] **Step 1: Failing-тесты `tests/exports.rs`** (натив, вызывают функции как обычные Rust через `rlib`):
  - `process_zero_and_oversized_frames_are_safe`: `engine_process(0)` ничего не меняет; `engine_process(100000)` обрабатывает ровно `MAX_BLOCK`, паники нет, выход конечен.
  - `init_twice_does_not_leak_or_keep_state`: после тика проб-тона `engine_init` повторно; телеметрия `frame == 0`; число аллокаций между вторым и третьим `engine_init` (через `count_allocs`) не растёт.
  - `telemetry_size_matches_struct`: `engine_telemetry_size() as usize == size_of::<Telemetry>()`.
  - `command_kinds_round_trip`: `engine_command(3, 90.0, 0.0, 0.0, 0.0)` меняет интервал шагов ровно так, как `Command::Tempo(90.0)`.
- [ ] **Step 2: Запустить** `cargo test -p versum-wasm` → FAIL.
- [ ] **Step 3: Реализовать `lib.rs`:** единственный `static mut ENGINE: Option<Engine>` и два статических буфера `[f32; MAX_BLOCK]`; все `unsafe`-блоки сводятся к доступу к этим статикам из одного потока, комментарий с инвариантом «worklet однопоточный».
- [ ] **Step 4: Запустить** `cargo test -p versum-wasm` → PASS. Затем собрать `cargo build --release --target wasm32-unknown-unknown -p versum-wasm` и убедиться, что `target/wasm32-unknown-unknown/release/versum_wasm.wasm` существует и импортов у модуля нет (`wasm-objdump`/`node` `WebAssembly.Module.imports(...).length === 0`).
- [ ] **Step 5: Commit** `feat(wasm): extern "C" engine exports`.

---

### Task 6: TypeScript — протокол, AudioWorklet-хост, интеграционный тест

Риск: высокий — AudioWorklet, асинхронный обмен сообщениями, расхождение layout между Rust и TS.

**Files:**
- Create: `package.json`, `tsconfig.json`, `vite.config.ts` (root `web`, `base: '/next/'`, `build.outDir: '../dist/next'`), `web/audio/{protocol.ts,host.ts,worklet.ts}`, `web/audio/protocol.test.ts`

**Interfaces:**
- Consumes: экспорты wasm из Task 5.
- Produces (`protocol.ts`): `type Gesture = {x,y,z,pinch: number}`; `type Command = {kind: 'gesture', ...Gesture} | {kind:'world', id:number} | {kind:'seed', seed:number} | {kind:'tempo', bpm:number} | {kind:'hold', on:boolean} | {kind:'lock', on:boolean}`; `encodeCommand(c: Command): [number, number, number, number, number]`; `type Telemetry = {version, frame, peakL, peakR, droppedSteps, droppedCommands: number; steps: {index,note,velocity}[]}`; `decodeTelemetry(buf: ArrayBuffer): Telemetry`.
- Produces (`host.ts`): `class AudioHost { static async create(ctx: AudioContext, wasmUrl: string, seed: number): Promise<AudioHost>; send(c: Command): void; onTelemetry: (t: Telemetry) => void; connect(dest: AudioNode): void }`. Worklet `'versum-engine'`: компиляция `WebAssembly.Module` на главном потоке, передача через `processorOptions`; телеметрия уходит в порт каждые 8 блоков, после отправки вызывается `engine_ack`; команды, пришедшие до готовности, ставятся в очередь и применяются при старте.

- [ ] **Step 1: Инициализация:** `npm init`-эквивалент с зависимостями `vite`, `typescript`, `vitest`; скрипты `test` (сначала `cargo build --release --target wasm32-unknown-unknown -p versum-wasm`, затем `vitest run`).
- [ ] **Step 2: Failing-тест `protocol.test.ts`** — загружает собранный `.wasm` в node (`WebAssembly.instantiate`) и прогоняет реальный движок:
  - `layout_matches_rust`: `engine_init(48000, 1)`, `engine_telemetry_size()` равен константе `TELEMETRY_SIZE` из `protocol.ts`; `decodeTelemetry` после 4 блоков даёт `version === 1`, `frame === 512`.
  - `steps_arrive_at_exact_frame`: `engine_command(3, 120, …)`, 6000 сэмплов блоками по 128; `decodeTelemetry(...).steps` содержит `index` 1 с моментом, не дальше одного блока от 6000.
  - `ack_clears_steps`: после `engine_ack` `steps.length === 0`, `droppedSteps === 0`.
  - `encode_command_matches_wasm_kinds`: `encodeCommand({kind:'tempo', bpm:90})` → `[3, 90, 0, 0, 0]`, аналогично для остальных пяти видов.
- [ ] **Step 3: Запустить** `npm test` → FAIL.
- [ ] **Step 4: Реализовать `protocol.ts`** (смещения зашить константами, `TELEMETRY_SIZE`) и затем `worklet.ts`, `host.ts` по интерфейсу выше. Worklet-файл подключается через `new URL('./worklet.ts', import.meta.url)` с проверкой, что Vite его собирает отдельным модулем.
- [ ] **Step 5: Запустить** `npm test` → PASS.
- [ ] **Step 6: Commit** `feat(web): protocol, audio host, AudioWorklet processor`.

---

### Task 7: Интерфейс скина, отладочный скин, ввод

Риск: обычный — графика и жесты на синтетических данных, без звука.

**Files:**
- Create: `web/gfx/{skin.ts,debug-skin.ts,synthetic.ts}`, `web/input/gesture.ts`, `web/gfx/debug-skin.test.ts`, `web/input/gesture.test.ts`

**Interfaces:**
- Consumes: `Telemetry`, `Gesture` из Task 6.
- Produces:
  - `interface Controls { x: number; y: number; z: number; pinch: number }`
  - `interface Skin { frame(ctx: CanvasRenderingContext2D, dt: number, t: Telemetry, c: Controls): void; note(index: number, velocity: number): void }`
  - `class DebugSkin implements Skin` — рисует пики столбцами и вспышку по `note`.
  - `syntheticTelemetry(frame: number): Telemetry` — детерминированный генератор, включая шаги.
  - `class GestureTracker { constructor(emit: (g: Gesture) => void); down(x,y,w,h): void; move(x,y,w,h): void; up(): void }` — переводит пиксели в оси [0,1]; `z` — секунды удержания.

- [ ] **Step 1: Failing-тесты:**
  - `debug_skin_runs_on_synthetic_telemetry_without_audio`: прогнать 600 кадров `DebugSkin.frame` на поддельном `CanvasRenderingContext2D` (записывающий `fillRect` и подобное) — нет исключений, вызовов рисования больше 0; в модуле нет обращений к `AudioContext` (тест импортирует только `skin.ts`, `debug-skin.ts`, `synthetic.ts`).
  - `gesture_axes_are_normalized_and_clamped`: касание за границей даёт значения в [0,1].
  - `gesture_z_counts_only_while_held`: `z` растёт между `down` и `up`, после `up` сбрасывается.
- [ ] **Step 2: Запустить** `npx vitest run web/gfx web/input` → FAIL.
- [ ] **Step 3: Реализовать** модули по интерфейсам выше.
- [ ] **Step 4: Запустить** тесты → PASS.
- [ ] **Step 5: Commit** `feat(web): skin interface, debug skin, gesture tracker`.

---

### Task 8: Страница `/next/`, сборка, Vercel, замер, проверка на iPhone

Риск: обычный — склейка и конфиги; ручная приёмка.

**Files:**
- Create: `web/index.html`, `web/app/main.ts`, `scripts/build.sh`, `scripts/vercel-install.sh`, `vercel.json`
- Modify: `README.md` (раздел «Сборка»)

**Interfaces:**
- Consumes: `AudioHost`, `DebugSkin`, `GestureTracker`.
- Produces: страница `/next/` с кнопкой «start» (жест пользователя → `AudioContext.resume()`), canvas с отладочным скином, перетаскивание передаёт `Gesture` в движок; параметр `?v=2` отображается в углу.

- [ ] **Step 1: `scripts/build.sh`** (`set -euo pipefail`): `cargo build --release --target wasm32-unknown-unknown -p versum-wasm`; копирует `.wasm` в `web/public/engine.wasm`; `npx vite build`; `cp -R legacy/. dist/`. `scripts/vercel-install.sh`: ставит `rustup` без интерактива, если `cargo` нет, добавляет таргет `wasm32-unknown-unknown`, `npm ci`. `vercel.json`: `installCommand`, `buildCommand`, `outputDirectory: "dist"`.
- [ ] **Step 2: Failing-проверка сборки:** `bash scripts/build.sh` на чистом клоне (удалить `dist/`, `target/`) → ожидаемо падает до написания `web/index.html`.
- [ ] **Step 3: Написать `web/index.html` и `web/app/main.ts`** по интерфейсу выше.
- [ ] **Step 4: Запустить** `bash scripts/build.sh` → успех; затем проверить `ls dist/index.html dist/next/index.html dist/next/engine.wasm dist/sw.js`, все четыре файла на месте.
- [ ] **Step 5: Локальный прогон:** `python3 -m http.server 8765 -d dist`; открыть `/` (legacy работает, `sw.js` не трогает `/next/`) и `/next/` (после «start» слышен тон, перетаскивание меняет высоту, отладочный скин показывает пики и вспышки шагов).
- [ ] **Step 6: Раздел «Сборка» в `README.md`:** команды `npm test`, `bash scripts/build.sh`, откуда берётся `versum-dsp`.
- [ ] **Step 7: Commit** `feat: /next/ probe page, build scripts, vercel config`.
- [ ] **Step 8 (с подтверждения пользователя): запушить ветку**, дождаться превью-деплоя Vercel, записать время сборки в `docs/superpowers/specs/…-design.md` (§6) и решить по спеке: остаёмся на Vercel или переносим сборку в GitHub Actions.
- [ ] **Step 9 (вручную пользователем): проверка на iPhone Safari** по превью-адресу: `/next/` запускается, тон слышен, перетаскивание меняет высоту, при блокировке экрана и возврате звук продолжается или корректно возобновляется. Результат записать в журнал плана; от него зависит переход к плану этапа 3.
