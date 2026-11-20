<p align="center"><img src="assets/logo.jpg" alt="Afflictio" width="180"></p>

# Afflictio

Мобильная ОС на ядре Linux (AArch64) с пространством пользователя на Rust. Ядро берется из upstream Linux и собирается с фрагментом конфигурации репозитория, все системные компоненты написаны с нуля и не зависят от systemd.

[![ci](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml/badge.svg)](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

[English](README.en.md)

## Состояние

Этапы 1-7 дорожной карты закрыты (теги v0.1.0 - v0.7.0). Каркас загружается в QEMU целиком: liinit монтирует ФС и запускает службы, tiosessd поднимает две autologin-сессии на VT1 и VT2 с реальным переключением на uid пользователя, комбинация Ctrl+Alt+Fn переключает консоли через клавиатурный слушатель на HAL, убитый демон перезапускается супервизором, SIGTERM приводит к корректному poweroff. tiocompd рисует композиторный кадр напрямую через KMS/DRM (page-flip на dumb-буферах), liobinder закрывает транспортный слой Binder, lioatd ведет AT-диалог с модемом (probe 27.007: модель, SIM, сигнал, регистрация). Собран дистрибутивный rootfs (alpine-база, пакеты Afflictio, пользователи, два дерева модулей, манифест), который загружается в QEMU с маркерами `SMOKE_UID_OK`, `SMOKE_KMS_FLIPS_OK` и `SMOKE_MODEM_AT_OK` (заглушка EG25-G на втором PL011) в job `rootfs`; модель обновлений решена как A/B-слоты: образ с boot-разделом и двумя rootfs-слотами, liota переключает слоты через extlinux, smoke проверяет переключение двумя загрузками (`SMOKE_AB_SWITCH_OK`, `SMOKE_AB_BOOT_B_OK`), flashable-образы для PinePhone (MBR, u-boot на смещении 8 КБ, три раздела, проверка через debugfs) собираются в job `device`. Прошивка на железо остается за bring-up, план работ в docs/roadmap.ru.md.

## Компоненты

| Компонент | Назначение |
| --- | --- |
| `liinit` | PID 1: монтирование, супервизор служб, корректное выключение |
| `tiosessd` | Менеджер сессий: autologin на несколько VT, переключение Ctrl+Alt+Fn, IPC |
| `tioctl` | CLI: статус, устройства, запуск сессий, poweroff и reboot |
| `tiohal` | Библиотека: evdev с разбором Ctrl+Alt+Fn, power_supply, backlight |
| `lioipc` | Общий протокол: JSON-сообщения для tiosessd и tioctl |
| `lioatd` | Модемный демон: AT-канал на termios, probe 27.007, фолбэк по списку устройств |
| `modemctl` | CLI к lioatd: снимок состояния и произвольные AT-команды |
| `liota` | A/B-обновления: статус слотов, переключение через extlinux, установка payload |
| `tiocomp` | Демон `tiocompd`: композитор поверх KMS/DRM, dumb-буферы, page-flip |
| `liobinder` | Транспорт Binder: парцеллы, `BINDER_WRITE_READ`, клиент servicemanager |

Имена собраны из кусочков слова Afflictio: `li` + init, `tio` + hal и так далее. Конфиги живут в `/etc/afflictio/`, сокет в `/run/afflictio/tiosess.sock`, параметр ядра называется `afflictio.init=`.

## Сборка

Нужен любой актуальный Rust (крейты держат минимальный набор зависимостей: `libc`, `serde`, `toml`, `signal-hook`):

```sh
cargo build --release
```

Бинарные артефакты: `target/release/liinit`, `target/release/tiosessd`, `target/release/tioctl`.

Под целевую архитектуру крейты собираются musl-таргетом со статической линковкой через `rust-lld`, кросс-gcc не требуется:

```sh
rustup target add aarch64-unknown-linux-musl
cargo build --release --target aarch64-unknown-linux-musl
```

## Запуск в QEMU

Smoke-тест делает весь путь. Ядро `linux-virt` берется из apk-пакета Alpine, оттуда же в initramfs попадают модули `virtio-gpu`, `evdev` и `virtio-input` (fbcon и VT встроены в ядро). Бинари собираются musl-таргетом, initramfs упаковывается из Alpine minirootfs. QEMU запускается с `virtio-gpu-pci` и `virtio-keyboard-pci`, клавиатура отправляется через QMP-монитор командой send-key. Внутри машины probe-скрипт проверяет статус (две сессии, счетчик клавиатур больше нуля), переключает консоли Ctrl+Alt+F1 и Ctrl+Alt+F2 и сравнивает содержимое `/dev/fb0` между переключениями: кадры обязаны отличаться, это доказывает, что текст сессий реально попадает на экран. Затем убивается tiosessd, супервизор его перезапускает, SIGTERM приводит машину к poweroff, QEMU завершается с кодом 0.

```sh
./scripts/qemu-smoke.sh
```

Требования к окружению: `qemu-system-aarch64`, `cpio`, `curl`, `python3`, `cargo`. Аргументы окружения: `KERNEL_APK_URL`, `MINIROOTFS_URL` (по умолчанию Alpine 3.22), `QEMU_DATA_DIR` (каталог прошивок QEMU, если ROM-файлы лежат нестандартно). Артефакты остаются в `out/`: ядро, модули, initramfs, serial-лог.

Сборка ядра из исходников с нашим фрагментом конфигурации описана в [kernel/README.md](kernel/README.md).

## Устройства

Первый target: [PinePhone](devices/pinephone/README.md). Скрипты этапа 3 тянут артефакты пайплайна postmarketOS (ядро 6.17.5, DTB, 431 модуль, U-Boot) с пинованных apk-пакетов, извлекают точный конфиг ядра из встроенного IKCONFIG-потока и сверяют с ним 19 требований нашего userspace, затем упаковывают boot-каталог, U-Boot и rootfs.ext4 в бандл для SD-карты. Конвейер проверяется в CI, прошивка на железо относится к bring-up.

## Запуск без QEMU

Каркас работает и без эмуляции. В терминале:

```sh
cargo run -p liinit
```

liinit заметит, что он не PID 1, пропустит монтирование и будет работать как обычный супервизор служб из своего TOML-конфига. Демон сессий запускается так же, путь сокета переопределяется переменной `AFFLICTIO_SOCKET`:

```sh
cargo run -p tiosess
AFFLICTIO_SOCKET=/tmp/tiosess.sock ./target/release/tioctl status
```

Пример конфига с двумя autologin-сессиями и списком служб лежит в kernel/initramfs/.

## Документация

- [Архитектура (RU)](docs/architecture.ru.md) / [Architecture (EN)](docs/architecture.en.md)
- [Дорожная карта (RU)](docs/roadmap.ru.md) / [Roadmap (EN)](docs/roadmap.en.md)
- [Ядро](kernel/README.md)
- [PinePhone](devices/pinephone/README.md) / [PinePhone (EN)](devices/pinephone/README.en.md)

## Лицензия

GPL-3.0-or-later, полный текст в [LICENSE](LICENSE).
