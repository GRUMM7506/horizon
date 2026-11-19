# Порт на Android (arm64-v8a)

Документ форка: что нужно Horizon, чтобы работать на Android, что для этого изменено, как собрать и что ещё
не проверено. Апстрим Android не поддерживает; всё Android-специфичное спрятано за
`#[cfg(target_os = "android")]`, новым крейтом `crates/horizon-android` и папкой `android/`.

Ход работы по этапам: [PROGRESS.md](../PROGRESS.md).

## 1. Разведка

### Что уже есть в апстриме

Апстрим по частям переносит к себе код стороннего Android-порта
[Starling](https://github.com/starling-client/starling) (ветка `android`, см. `THIRD_PARTY_NOTICES.md`). В дереве
уже лежит чистая логика без единого обращения к ОС, покрытая тестами, но **нигде не подключённая к игре**:

| Файл (`crates/horizon-client-render/src/`) | Что делает |
|---|---|
| `touch.rs`, `touch/{layout,aim,draw}.rs` | Сенсорное управление: джойстик или D-pad, жесты камеры, кнопки, тап и удержание по миру, раскладки, размер и прозрачность кнопок, отрисовка в HUD. Пальцы превращаются в `PadState`, то есть в то же состояние, что даёт геймпад |
| `android_gamepad.rs` | Декодер геймпада из кодов Android (`KEYCODE_BUTTON_*`, `AXIS_*`) в `PadState`, выбор активного устройства ввода |
| `android_runtime.rs` | Глобальное состояние ввода на процесс: геймпады, режим ввода, keypad-профиль |
| `keypad.rs` | Профиль для кнопочных телефонов |

Порт подключает эти модули, а не пишет своё управление: так форк проще ребейзить, а когда апстрим подключит их
сам, конфликт будет в нескольких строках.

### Платформенно-зависимый код

**Запуск.** `horizon_game::main` → `horizon_guest_stack::prepare_main` → `entry::prepare` →
`horizon_mod_host::main_with` → `entry::start` → `game::run_with`.

| Место | Проблема на Android | Решение |
|---|---|---|
| `horizon-guest-stack/src/lib.rs` `prepare_main`, `inspect_current` | На Linux процесс перезапускает сам себя с большим стеком; на «прочих» ОС проверка стека возвращает ошибку и процесс завершается. Потоку игры нужно 136 МиБ стека (гостевой код модов, большие структуры) | Проверка стека для Android та же, что для Linux (`pthread_getattr_np`); поток `android_main` создаётся с нужным стеком (см. раздел 3) |
| `entry.rs` `prepare()` | Читает `argv` и `current_exe`; модуль `horizon_mod_package::holder` выключен вне Linux/macOS/Windows, крейт не собирается | Отдельный `prepare_android(args, game_dir, home)`: аргументы приходят от лаунчера |
| `launch_options.rs` `from_launch` | Ресурсы по умолчанию ищутся рядом с исполняемым файлом | Лаунчер всегда передаёт `--resources`, `--game-dir`, `--home` |
| `std::process::exit` при ошибках | Убивает процесс активности | Игра работает в отдельном процессе `:game`; лаунчер показывает хвост лога, если игра завершилась не сама |
| `horizon-mod-wasm/src/module_cache.rs` | Компиляция WASM-модов запускает дочерний процесс из `current_exe` | Не поддерживается: моды на Android пока не работают (известное ограничение) |
| `desktop.rs`, `pad_menu.rs` (`xdg-open`) | Открытие ссылок и папок | Не работает, не падает |

**Окно и поверхность.** winit 0.30.13, один цикл событий на процесс; три места создают его через
`EventLoop::new()`: `game/runtime/resource_loader.rs` (обычный старт), `loading_screen.rs` (синхронный путь),
`no_resources.rs` (нет ресурсов). На Android циклу нужен `AndroidApp`. Обработчиков `ApplicationHandler` четыре:
`App` (экран загрузки → игра), `Split` (разделённый экран), `Game`, `Missing`. Окно создаётся в `resumed`, но
`suspended` не реализован нигде, а повторный `resumed` игнорируется. В `Renderer` (`horizon-client-render/src/gfx.rs`)
уже есть `recreate_surface()` (на том же устройстве) и флаг `suspended`, при котором кадр пропускается: этого
достаточно, чтобы отпускать поверхность при сворачивании и создавать новую при возврате.

Расширения winit, которых нет на Android: `platform::scancode`, `platform::modifier_supplement`
(`horizon-client-input/src/keybinds.rs`, `game/runtime/window_handler.rs`), `platform::wayland` (уже за
`cfg(target_os = "linux")`, а Android это не `linux`).

**Графика.** wgpu 30.0.1 со своей копией `wgpu-hal`. Порядок бэкендов вне Windows: Vulkan, затем GL. Все
возможности устройства запрашиваются по маске `adapter.features()`, лимиты берутся у адаптера, так что жёстких
требований десктопного уровня нет. Обязательны: `Depth32Float` с чтением, достаточное число слоёв массива
текстур, формат поверхности. Для Adreno в `gfx/cull.rs` уже есть отдельная ветка.

**Ввод.** `WindowEvent::Touch` не обрабатывается. Клавиатура и мышь идут через
`Game::handle_window_event_inner` (`game.rs`); геймпад читается в `game/interface/pad/mod.rs` (`poll_pads`),
куда `horizon-ctl pad` уже подмешивает виртуальный пад. Текст приходит через `Ime::Commit` и `KeyEvent.text`.
winit на Android с GameActivity не передаёт текст экранной клавиатуры (событие `TextEvent` он отбрасывает), а
оси геймпада превращает в ложные касания.

**Звук.** `cpal` 0.16 в `horizon-client-render` (фича `audio`); у cpal есть бэкенд AAudio, зависимость уже в
`Cargo.lock`. Формат и частота берутся по умолчанию у устройства, нет устройства: игра молчит и работает.
Приостановки потока нет. Декодер (`symphonia`) на чистом Rust. Диктор (`horizon-narrator`) на неизвестной ОС
просто выключен.

**Файлы.** Крейтов `dirs`/`home` нет. Домашняя папка: `--home`, затем `HORIZON_HOME`, затем `XDG_DATA_HOME`/`HOME`.
Папка игры (`--game-dir`) становится рабочим каталогом процесса, `options.txt` и сохранения лежат в ней.
Масштаб интерфейса и дальность прорисовки хранятся в `options.txt` (`guiScale`, `renderDistance`); для дальности
есть и флаг `--render-distance`.

**Архитектура процессора.** x86-специфики без запасного пути нет: macOS ARM поддерживается официально, так что
у всего (`jmath`, AES в протоколе, раскрутка стека в `horizon-threads`) есть ветка aarch64 или переносимая.
`horizon-threads` уже знает про `target_os = "android"` (сигналы, память процесса, дамп стеков).

**Зависимости, которые не собираются под Android.**

| Зависимость | Что с ней |
|---|---|
| `arboard` (буфер обмена) | Нет бэкенда: вынесена из Android-сборки, копирование работает внутри игры |
| `winit` | Требует выбрать `android-game-activity` или `android-native-activity` |
| `hidapi` (`horizon-dualsense`), `gilrs` | Фича `controllers`: на Android выключена, геймпад идёт через Java и `android_runtime` |
| `mimalloc` | Собирается NDK; можно заменить на системный (`system-allocator`) |
| `wasmtime` (`horizon-mod-host`) | Собирается, но моды не запускаются (см. выше) |

### План

1. Сборка `horizon-client` под `aarch64-linux-android`; новый крейт `crates/horizon-android` (`cdylib`,
   `android_main`, JNI). Точка входа живёт не в `horizon-client`: загрузчик модов (`horizon-mod-host`) стоит
   выше движка по слоям (`docs/layers.md`), а тип крейта в Cargo нельзя задать только для одной платформы.
2. Жизненный цикл: `suspended` отпускает поверхность, `resumed` создаёт новую на том же устройстве.
3. Звук: cpal/AAudio, остановка потока при сворачивании.
4. Сенсорное управление через `TouchControls`; геймпад и текст с экранной клавиатуры через JNI.
5. Пути: лаунчер передаёт папку `getExternalFilesDir()/horizon`.
6. Лаунчер на Kotlin (`android/`), CI с debug-APK.
