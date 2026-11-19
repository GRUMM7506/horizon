# Порт на Android: ход работы

Ветка `android-port`. Подробности: [docs/android-port.md](docs/android-port.md).

## Этап 1. Разведка: готово

- Изучены workspace, запуск, окно и поверхность, ввод, звук, пути, зависимости. Находки:
  [docs/android-port.md](docs/android-port.md#1-разведка).
- Главная находка: в апстриме уже лежат (но не подключены к игре) модули сенсорного управления и Android-ввода,
  перенесённые из стороннего порта Starling. Порт подключает их, а не пишет управление заново.
- Android SDK, NDK r27c и cargo-ndk в песочнице ставятся и работают, так что сборка проверяется локально, а не
  только в CI.

## Этап 2. Сборка: готово

- `cargo ndk -t arm64-v8a build -p horizon-client` проходит (rlib).
- Новый крейт `crates/horizon-android`: `cdylib` с `android_main` (android-activity, GameActivity).
  `libhorizon_android.so` собирается и линкуется; проверено, что экспортированы `android_main`,
  `GameActivity_onCreate`, JNI-функции моста, сегменты выровнены на 16 КБ.
- Решение, отличающееся от формулировки задачи: `cdylib` собирается из отдельного крейта, а не из
  `horizon-client`. Причины: загрузчик модов стоит выше движка по слоям (`docs/layers.md`), и тип крейта нельзя
  включить только для Android (на десктопе пришлось бы каждый раз линковать лишнюю библиотеку).
- Поток `android_main` получает 136 МиБ стека через `--wrap=pthread_create` (без вендоринга android-activity);
  по дизассемблеру проверено, что glue-код зовёт обёртку.
- Не собирались под Android и обойдены: `arboard`, расширения winit `scancode` и `modifier_supplement`,
  модуль `holder`.
