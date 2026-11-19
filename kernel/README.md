# Ядро Afflictio

Ядро системы берется из upstream Linux (ветка 6.12 LTS, архитектура AArch64) и собирается поверх `defconfig` с фрагментом `configs/afflictio.fragment`. Сам исходный код ядра в репозиторий не вендорится: сборка скачивает архив выбранной версии с kernel.org и применяет наш фрагмент через `scripts/kconfig/merge_config.sh`.

## Что включает фрагмент

- Базовая платформа ARM64, SMP, вытесняемость, high-res таймеры.
- Драйверы, без которых каркас не поднимается: `devtmpfs`, `tmpfs`, evdev, VT-консоль, `simpledrm` для вывода, классы backlight и power_supply, которые читает HAL.
- Binder IPC и binderfs для будущей интеграции с Android HAL через libgbinder.
- initramfs, ELF и script binfmt для загрузки userspace.

## Сборка

Кросс-компиляция на x86_64-хосте требует тулчейна `aarch64-linux-gnu-`:

```sh
LINUX_VERSION=6.12 ./scripts/build-kernel.sh
```

Параметры окружения: `LINUX_VERSION` (по умолчанию 6.12), `CROSS_COMPILE`, `JOBS`, `OUTPUT_DIR` (по умолчанию `out/`, в репозиторий не попадает). Результат: `out/linux-*/arch/arm64/boot/Image`, dtbs и модули.

## Планы по ядру

Путь к конкретным устройствам лежит через downstream-ядра вендоров и патчи postmarketOS: они содержат рабочие DTS и драйверы для камер, модемов и Wi-Fi, которых нет в mainline. Отдельное направление: выработка унифицированного набора патчей, который накатывается на разные вендорские ядра одной командой. По userspace-мостам (libhybris для вызова Android-заголовков Bionic, libgbinder для Binder-вызовов) см. docs/roadmap.md.

## Почему фрагмент, а не форк

Форк ядра моментально отрывается от upstream и превращает обновление безопасности в ручную работу. Фрагмент конфигурации хранит только отличие Afflictio от базового defconfig, поэтому пересборка на новой версии Linux сводится к подмене одного аргумента скрипта.
