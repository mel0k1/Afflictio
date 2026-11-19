<p align="center"><img src="assets/logo.jpg" alt="Afflictio" width="180"></p>

# Afflictio

Мобильная ОС на ядре Linux (AArch64) с пространством пользователя на Rust. Ядро берется из upstream Linux и собирается с фрагментом конфигурации репозитория, все системные компоненты написаны с нуля и не зависят от systemd.

[![ci](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml/badge.svg)](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

[English](README.en.md)

## Состояние

Каркас уже загружается в QEMU целиком: liinit монтирует ФС и запускает службы, tiosessd поднимает autologin-сессию на виртуальном терминале, tioctl общается с демоном по unix-сокету, убитый демон перезапускается супервизором, SIGTERM в PID 1 приводит машину к корректному poweroff. Этот сценарий выполняется и локально, и в CI (job `qemu`). Этап 1 дорожной карты закрыт тегом v0.1.0. Графический стек и образы для конкретных устройств впереди, план работ в docs/roadmap.ru.md.

## Компоненты

| Компонент | Назначение |
| --- | --- |
| `liinit` | PID 1: монтирование, супервизор служб, корректное выключение |
| `tiosessd` | Менеджер сессий: autologin, VT-переключение, IPC на unix-сокете |
| `tioctl` | CLI: статус, устройства, запуск сессий, poweroff и reboot |
| `tiohal` | Библиотека: evdev, power_supply, backlight |
| `lioipc` | Общий протокол: JSON-сообщения для tiosessd и tioctl |

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

Smoke-тест делает весь путь: собирает aarch64-бинари, упаковывает initramfs из Alpine minirootfs, скачивает ядро `linux-virt` из netboot-репозитория Alpine, вынимает payload из EFI zboot-обёртки и грузит всё в `qemu-system-aarch64 -machine virt`. Внутри liinit запускает tiosessd с autologin, проверяющий скрипт ходит в IPC, убивает tiosessd и дожидается его перезапуска супервизором, затем отправляет SIGTERM в PID 1. Машина уходит в poweroff, QEMU завершается с кодом 0, в serial-логе остаются маркеры `SMOKE_IPC_OK`, `SMOKE_SESSIONS_OK`, `SMOKE_RESPAWN_OK`.

```sh
./scripts/qemu-smoke.sh
```

Требования к окружению: `qemu-system-aarch64`, `cpio`, `curl`, `cargo`. Аргументы окружения: `KERNEL_URL`, `MINIROOTFS_URL` (по умолчанию Alpine 3.22), `QEMU_TIMEOUT`, `QEMU_DATA_DIR` (каталог прошивок QEMU, если ROM-файлы лежат нестандартно). Артефакты остаются в `out/`: ядро, initramfs, serial-лог.

Сборка ядра из исходников с нашим фрагментом конфигурации описана в [kernel/README.md](kernel/README.md).

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

Пример конфигов с autologin и списком служб лежит в kernel/initramfs/.

## Документация

- [Архитектура (RU)](docs/architecture.ru.md) / [Architecture (EN)](docs/architecture.en.md)
- [Дорожная карта (RU)](docs/roadmap.ru.md) / [Roadmap (EN)](docs/roadmap.en.md)
- [Ядро](kernel/README.md)

## Лицензия

GPL-3.0-or-later, полный текст в [LICENSE](LICENSE).
